//! Compiler artifact publication for cooperating local writers.
//!
//! The lock and bounded ownership registry coordinate fe2o3 processes that use this protocol.
//! Producer source paths and registry contents are non-authoritative cleanup hints, not
//! authenticated identities or proof that an artifact is valid. The compiler subprocess receives
//! staged paths through an inherited `/proc/self/fd` staging-directory handle; this pins the exact
//! private staging inode across pathname substitution, but is Linux-specific and does not
//! constrain a hostile subprocess.
//!
//! Cargo-managed builds also use a bounded attempt registry. A generation and random build
//! session are recorded before old producer outputs are invalidated; a backend may publish only
//! while that exact generation is current. Both registries use the same pinned directory and
//! exclusive lock. Attempt state prevents stale cooperating compilers from publishing, but it is
//! coordination metadata rather than artifact or launch authority.
//! On Linux, a canonical `/proc/self/fd/<n>` output root is imported by duplicating that descriptor;
//! every ordinary configured path is still opened one component at a time without following
//! symlinks.
//!
//! The configured output directory is a generated-artifact namespace. Canonically named files
//! without a registry owner are treated as legacy fe2o3 outputs: a successful transaction adopts
//! them, while a failed transaction invalidates them so stale executable code cannot survive a
//! rejected codegen preflight or rebuild. An entry explicitly owned by another producer is never
//! adopted or removed. Fully absent ownership entries are pruned as crash tombstones before name
//! protection is applied.
//!
//! Staged files and directories are synced before publication, and the output directory is synced
//! after registry commit and staging cleanup. Each final rename is atomic, but the collection is
//! not atomically visible as a unit: a crash during the rename sequence can leave a partial
//! generation, which a later cooperating transaction will reconcile.
//!
//! Successful transactions return immutable IR and code-object snapshots read through the exact
//! staged file descriptors after publication and before releasing the lock. Returned paths are
//! diagnostics only, so later publication at the same names cannot change an earlier result.
//!
//! # Filesystem concurrency contract
//!
//! The output directory is a private protocol namespace for cooperating fe2o3 writers. Every
//! writer that can create, rename, replace, or remove entries in that directory must use this
//! crate's composite Linux lock. Writers split across mount namespaces must explicitly configure
//! one shared, pre-provisioned guard directory with `FE2O3_ARTIFACT_PATH_GUARD_DIR` and its exact
//! inode identity. That guard object is locked as one namespace-independent domain, so different
//! absolute aliases cannot split the critical section. Writers restricted to one mount namespace
//! must explicitly select normalized-absolute-path byte-range coordination with
//! [`enable_same_mount_namespace_artifact_path_guard_v1`]. A named-file OFD record lock preserves
//! interoperability with existing cooperating writers, and a
//! descriptor-owned lock on the root inode prevents replacement of the named lock from creating a
//! second critical section. Closing unrelated descriptors cannot release these locks. Lock
//! descriptors are `CLOEXEC`, but a forked child retains inherited locks until it closes those
//! descriptors or successfully executes; pre-exec child code must not re-enter these APIs. Linux
//! has no
//! unlink-by-file-descriptor operation. Consequently, these APIs detect substitutions observed
//! before a destructive
//! operation and verify their results, but they cannot prevent arbitrary same-UID code that
//! ignores the lock from replacing a pathname in the final check-to-unlink interval. Callers must
//! not expose the directory to such writers. This is a coordination boundary, not a defense
//! against a malicious process running as the artifact-store owner.

mod attempt;
mod attempt_scoped_hsaco_publication;
mod compiler_artifact_generation_v1;
mod compiler_module_handoff;
mod durable_link_publication;
mod durable_published_claim;
mod link_publication;
mod managed_invocation_capability;
mod retained_durable_directory;
mod worker_v2_publication_intent;
mod worker_v3_load_readiness;
mod worker_v3_publication_binding;
mod worker_v3_publication_intent;

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub use attempt::{
    AttemptCodecError, BackendPublicationReceiptV1, BackendPublicationReceiptV2,
    BackendPublicationReceiptV3, BuildAttempt, BuildInvocation, BuildSession,
    SimulationObservationReceiptV1,
};
use attempt::{AttemptPhase, AttemptRegistry, MAX_ATTEMPT_BYTES, StartAttemptOutcome};
pub use attempt_scoped_hsaco_publication::{
    AttemptScopedHsacoPublicationBoundaryV2, AttemptScopedHsacoPublicationErrorV1,
    AttemptScopedHsacoPublicationErrorV2, AttemptScopedHsacoPublicationErrorV3,
    AttemptScopedHsacoPublicationFaultPointV2, AttemptScopedHsacoPublicationFaultTimingV2,
    AttemptScopedHsacoPublicationOptionsV2, AttemptScopedHsacoPublicationOutcomeV1,
    AttemptScopedHsacoPublicationOutcomeV2, AttemptScopedHsacoPublicationOutcomeV3,
    AttemptScopedHsacoPublicationResultV1, AttemptScopedHsacoPublicationResultV2,
    AttemptScopedHsacoPublicationResultV3, BackendPublicationReceiptValidationErrorV1,
    BackendPublicationReceiptValidationErrorV2, BackendPublicationReceiptValidationErrorV3,
    PersistedBackendReceiptV1, PersistedBackendReceiptV2, PersistedBackendReceiptV3,
    UpstreamCodeObjectEvidenceIdentityV1, VerifiedWorkerV3PublicationAuthorityV1,
    producer_package_identity_v1, publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v1_with_options,
    publish_exact_hsaco_evidence_for_attempt_v2,
    publish_exact_hsaco_evidence_for_attempt_v2_with_options,
    publish_exact_hsaco_evidence_for_attempt_v3,
    publish_exact_hsaco_evidence_for_attempt_v3_with_options, read_backend_publication_receipt_v1,
    read_backend_publication_receipt_v2, read_backend_publication_receipt_v3,
    recover_published_hsaco_claim_for_attempt_v1, recover_published_hsaco_claim_for_attempt_v2,
    recover_published_hsaco_claim_for_attempt_v3, validate_backend_publication_receipt_v1,
    validate_backend_publication_receipt_v2, validate_backend_publication_receipt_v3,
};
pub use compiler_artifact_generation_v1::{
    CompilerArtifactGenerationErrorV1, CompilerArtifactGenerationFaultPointV1,
    CompilerArtifactGenerationFaultTimingV1, CompilerArtifactGenerationLeaseV1,
    CompilerArtifactGenerationManifestEntryV1, CompilerArtifactGenerationManifestIdentityV1,
    CompilerArtifactGenerationManifestV1, CompilerArtifactGenerationObjectBoundaryV1,
    CompilerArtifactGenerationObjectV1, CompilerArtifactGenerationObservationV1,
    CompilerArtifactGenerationOptionsV1, CompilerArtifactGenerationPublishOutcomeV1,
    CompilerArtifactGenerationQuotaV1, CompilerArtifactGenerationReclamationV1,
    CompilerArtifactGenerationRecordBoundaryV1, CompilerArtifactGenerationRecordOperationV1,
    CompilerArtifactGenerationRequestV1, CompilerArtifactGenerationScopeV1,
    CompilerArtifactGenerationStoreV1, CompilerArtifactRoleV1,
    DEFAULT_COMPILER_ARTIFACT_STORE_BYTES_V1, DEFAULT_COMPILER_ARTIFACT_STORE_ENTRIES_V1,
    HARD_MAX_COMPILER_ARTIFACT_STORE_BYTES_V1, HARD_MAX_COMPILER_ARTIFACT_STORE_ENTRIES_V1,
    MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1, MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1,
    MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1, MAX_COMPILER_HSACO_BYTES_V1,
    MAX_COMPILER_LINEAGE_BYTES_V1, MAX_COMPILER_NEUTRAL_KIR_BYTES_V1,
    MAX_COMPILER_SEMANTIC_MIR_BYTES_V1, MAX_COMPILER_TARGET_KIR_BYTES_V1,
};
pub use compiler_module_handoff::{
    CompilerModuleHandoffConsumptionTokenV3, CompilerModuleHandoffCurrentnessLeaseV3,
    CompilerModuleHandoffErrorV1, CompilerModuleHandoffErrorV2, CompilerModuleHandoffErrorV3,
    CompilerModuleHandoffIdentityV1, CompilerModuleHandoffIdentityV2,
    CompilerModuleHandoffPublicationV3, CompilerModuleHandoffReceiptV1,
    CompilerModuleHandoffReceiptV2, CompilerModuleHandoffReceiptV3, CompilerModuleHandoffSlotV1,
    CompilerModuleHandoffSlotV2, CompilerModuleHandoffSlotV3,
    CompilerModuleHandoffTransactionIdentityV3, ConsumedCompilerModuleHandoffV1,
    ConsumedCompilerModuleHandoffV2, ConsumedCompilerModuleHandoffV3,
    ConsumedSimulationKernelIrHandoffV1, MAX_COMPILER_MODULE_HANDOFF_BYTES,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3, SimulationKernelIrHandoffIdentityV1,
    SimulationKernelIrHandoffReceiptV1, SimulationKernelIrHandoffSlotV1,
    acquire_compiler_module_handoff_currentness_lease_v3, complete_simulation_kernel_ir_attempt_v1,
    consume_compiler_module_handoff_in_slot_v1, consume_compiler_module_handoff_in_slot_v2,
    consume_compiler_module_handoff_in_slot_v3, consume_compiler_module_handoff_v1,
    consume_compiler_module_handoff_v2, consume_compiler_module_handoff_v3,
    consume_compiler_module_handoff_with_currentness_v3, consume_simulation_kernel_ir_handoff_v1,
    publish_compiler_module_handoff_in_slot_v1, publish_compiler_module_handoff_in_slot_v2,
    publish_compiler_module_handoff_in_slot_v3,
    publish_compiler_module_handoff_in_slot_with_currentness_v3,
    publish_compiler_module_handoff_v1, publish_compiler_module_handoff_v2,
    publish_compiler_module_handoff_v3, publish_compiler_module_handoff_with_currentness_v3,
    publish_simulation_kernel_ir_handoff_v1, recover_compiler_module_handoff_receipt_in_slot_v3,
    recover_compiler_module_handoff_receipt_v3,
};
pub use durable_link_publication::{
    DurableArtifactBoundaryV1, DurableCurrentLinkPublicationLeaseV1,
    DurableCurrentLinkPublicationTokenV1, DurableFaultTimingV1, DurableJournalBoundaryV1,
    DurableJournalStageV1, DurableLinkPublicationError, DurableLinkPublicationFaultPointV1,
    DurableLinkPublicationOptionsV1, DurableLinkPublicationOutcomeV1, DurableLinkPublicationPlanV1,
    DurableLinkPublicationResultV1, DurableLinkPublicationSnapshotV1,
    DurableLinkPublicationTransactionV1, MAX_DURABLE_FINALIZED_ARTIFACT_BYTES,
    MAX_DURABLE_LINK_PUBLICATION_RECORD_BYTES, publish_durable_link_v1,
    publish_durable_link_v1_with_options, recover_durable_link_publication_v1,
};
pub use durable_published_claim::{
    DurablePublishedClaimCodecErrorV1, DurablePublishedClaimCodecErrorV2,
    DurablePublishedClaimCodecErrorV3, DurablePublishedClaimReacquisitionErrorV1,
    DurablePublishedClaimReacquisitionErrorV2, DurablePublishedClaimReacquisitionErrorV3,
    DurablePublishedClaimReceiptFieldV1, DurablePublishedClaimReceiptFieldV2,
    DurablePublishedClaimReceiptFieldV3, DurablePublishedClaimWorkerV3BindingFieldV1,
    DurablePublishedHsacoClaimV1, DurablePublishedHsacoClaimV2, DurablePublishedHsacoClaimV3,
    MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES, MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2,
    MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3, reacquire_current_hsaco_publication_lease_v1,
    reacquire_current_hsaco_publication_lease_v2, reacquire_current_hsaco_publication_lease_v3,
};
pub use link_publication::{
    AtomicPublicationIdentityV1, CanonicalLinkRequestIdentityV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, IdentityKindV1, InvalidationReasonV1, KernelSetIdentityV1,
    LinkPublicationCatalogV1, LinkPublicationCodecError, LinkPublicationPhaseV1,
    LinkPublicationRecordV1, LinkPublicationScopeV1, LinkPublicationStateV1,
    LinkedOutputIdentityV1, MAX_LINK_PUBLICATION_RECORD_BYTES, MAX_LINK_PUBLICATION_SCOPES,
    PackageIdentityV1, PinnedWorkerIdentityV1, PublicationOutcomeV1, PublishedLinkArtifactV1,
    RecoveryOutcomeV1, TargetIdentityV1, ValidatedResponseIdentityV1,
};
pub use managed_invocation_capability::{
    BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1, BROKERED_ARTIFACT_DIRECTORY_PATH_V1,
    BROKERED_CODEGEN_BACKEND_CHILD_FD_V1, BROKERED_CODEGEN_BACKEND_PATH_V1,
    BROKERED_INVOCATION_ADMITTED_V1, BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1,
    BROKERED_INVOCATION_PREPARED_V1, BROKERED_INVOCATION_REQUEST_BYTES_V1,
    BrokeredInvocationCapabilityClaimV1, BrokeredInvocationCapabilityCodecErrorV1,
    BrokeredInvocationCapabilityRequestV1,
};
pub use retained_durable_directory::{
    NoRetainedDurableDirectoryHooksV1, RetainedDurableArtifactBoundaryV1,
    RetainedDurableDirectoryErrorV1, RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1,
    RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1,
    RetainedDurableRecoveryBoundaryV1, RetainedDurableRecoveryMutationBoundaryV1,
};
use rustix::fd::{AsRawFd, FromRawFd, OwnedFd};
use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, fstat, fsync, mkdirat, open, openat, renameat, statat,
    unlinkat,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process;
#[cfg(feature = "test-hooks")]
use std::sync::Weak;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
pub use worker_v2_publication_intent::{
    MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1,
    MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES,
    MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES_V2,
    MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES,
    MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES_V2, RecoveredWorkerV2PublicationIntentV1,
    RecoveredWorkerV2PublicationIntentV2, WorkerV2PublicationIntentBoundaryV1,
    WorkerV2PublicationIntentBoundaryV2, WorkerV2PublicationIntentCleanupEscrowBoundaryV1,
    WorkerV2PublicationIntentCleanupEscrowCommitEvidenceV2,
    WorkerV2PublicationIntentCleanupEscrowErrorV1,
    WorkerV2PublicationIntentCleanupEscrowFaultPointV1,
    WorkerV2PublicationIntentCleanupEscrowFaultTimingV1,
    WorkerV2PublicationIntentCleanupEscrowIdentityV1,
    WorkerV2PublicationIntentCleanupEscrowOptionsV1, WorkerV2PublicationIntentCleanupEscrowStateV1,
    WorkerV2PublicationIntentCleanupEscrowV1, WorkerV2PublicationIntentErrorV1,
    WorkerV2PublicationIntentErrorV2, WorkerV2PublicationIntentFaultPointV1,
    WorkerV2PublicationIntentFaultPointV2, WorkerV2PublicationIntentFaultTimingV1,
    WorkerV2PublicationIntentFaultTimingV2, WorkerV2PublicationIntentIdentityV1,
    WorkerV2PublicationIntentIdentityV2, WorkerV2PublicationIntentLeaseV2,
    WorkerV2PublicationIntentOptionsV1, WorkerV2PublicationIntentOptionsV2,
    WorkerV2PublicationIntentOutcomeV1, WorkerV2PublicationIntentOutcomeV2,
    WorkerV2PublicationIntentRecordV1, WorkerV2PublicationIntentRecordV2,
    acquire_worker_v2_publication_intent_lease_v2, clear_worker_v2_publication_intent_v1,
    clear_worker_v2_publication_intent_v2,
    commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2,
    commit_worker_v2_publication_intent_cleanup_escrow_v1,
    commit_worker_v2_publication_intent_cleanup_escrow_v1_with_options,
    persist_worker_v2_publication_intent_v1, persist_worker_v2_publication_intent_v1_with_options,
    persist_worker_v2_publication_intent_v2, persist_worker_v2_publication_intent_v2_with_options,
    prepare_worker_v2_publication_intent_cleanup_escrow_v1,
    prepare_worker_v2_publication_intent_cleanup_escrow_v1_with_options,
    recover_worker_v2_publication_intent_cleanup_escrow_v1,
    recover_worker_v2_publication_intent_v1, recover_worker_v2_publication_intent_v2,
    rollback_worker_v2_publication_intent_cleanup_escrow_v1,
    rollback_worker_v2_publication_intent_cleanup_escrow_v1_with_options,
};
pub use worker_v3_load_readiness::{
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1, MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1,
    VerifiedWorkerV3LoadEnvelopeAuthorityV1, WorkerV3LoadEnvelopeBindingV1,
    WorkerV3LoadReadinessBoundaryV1, WorkerV3LoadReadinessCodecErrorV1,
    WorkerV3LoadReadinessErrorV1, WorkerV3LoadReadinessFaultPointV1,
    WorkerV3LoadReadinessFaultTimingV1, WorkerV3LoadReadinessOptionsV1,
    WorkerV3LoadReadinessOutcomeV1, WorkerV3LoadReadinessReceiptV1, WorkerV3LoadReadinessResultV1,
    discover_worker_v3_load_readiness_attempts_v1, publish_worker_v3_load_readiness_v1,
    publish_worker_v3_load_readiness_v1_with_options,
    recover_worker_v3_load_readiness_for_attempt_v1, recover_worker_v3_load_readiness_v1,
    scavenge_superseded_worker_v3_load_readiness_v1,
};
pub use worker_v3_publication_binding::{
    MAX_WORKER_V3_PUBLICATION_BINDING_BYTES_V1, WorkerV3PublicationBindingErrorV1,
    WorkerV3PublicationBindingIdentityFieldV1, WorkerV3PublicationBindingV1,
};
pub use worker_v3_publication_intent::{
    MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
    MAX_WORKER_V3_PUBLICATION_INTENT_CALLER_OWNER_CAPACITY_BYTES_V1,
    MAX_WORKER_V3_PUBLICATION_INTENT_METADATA_BYTES_V1,
    MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
    MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1,
    MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_ARCHIVE_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1, RecoveredWorkerV3PublicationIntentV1,
    WORKER_V3_PUBLICATION_INTENT_FINAL_ENTRY_HEADROOM_V1, WorkerV3ExternalProviderPayloadsV1,
    WorkerV3FinalizerReplayAttachmentsV1, WorkerV3PublicationIntentBoundaryV1,
    WorkerV3PublicationIntentCodecErrorV1, WorkerV3PublicationIntentErrorV1,
    WorkerV3PublicationIntentFaultPointV1, WorkerV3PublicationIntentFaultTimingV1,
    WorkerV3PublicationIntentIdentityV1, WorkerV3PublicationIntentInvalidReasonV1,
    WorkerV3PublicationIntentOptionsV1, WorkerV3PublicationIntentOutcomeV1,
    WorkerV3PublicationIntentRecordV1, WorkerV3PublicationIntentScavengeOutcomeV1,
    clear_worker_v3_publication_intent_v1, clear_worker_v3_publication_intent_v1_with_options,
    persist_worker_v3_publication_intent_v1, persist_worker_v3_publication_intent_v1_with_options,
    recover_worker_v3_publication_intent_v1, resume_worker_v3_publication_intent_retirement_v1,
    retire_worker_v3_publication_intent_after_load_readiness_v1,
    retire_worker_v3_publication_intent_after_load_readiness_v1_with_options,
    scavenge_worker_v3_publication_intent_occurrence_v1,
    scavenge_worker_v3_publication_intent_occurrence_v1_with_options,
};

/// Immutable bytes captured from one finalized artifact while its publication lock was held.
///
/// The path is diagnostic only. Consumers must use [`Self::bytes`] rather than reopening it: a
/// later transaction may legitimately publish a newer generation at the same path.
#[derive(Clone, Debug)]
pub struct FinalizedArtifactSnapshot {
    path: PathBuf,
    bytes: Arc<[u8]>,
}

impl FinalizedArtifactSnapshot {
    /// Constructs an immutable in-memory snapshot.
    ///
    /// Transaction results use the same representation after pinning and validating the exact
    /// published file. This constructor is useful for consumers that already own trusted bytes;
    /// it does not claim that `path` was transactionally published.
    pub fn from_bytes(path: impl Into<PathBuf>, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
        }
    }

    /// Returns the publication path for diagnostics only.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the exact bytes captured for this generation.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Generation-pinned files published for one successfully compiled kernel.
#[derive(Clone, Debug)]
pub struct DeviceArtifact {
    /// Canonical kernel artifact stem.
    pub kernel_name: String,
    /// Exact finalized LLVM IR captured by this transaction.
    pub llvm_ir: FinalizedArtifactSnapshot,
    /// Exact finalized AMDGPU code object captured by this transaction.
    pub hsaco: FinalizedArtifactSnapshot,
}

/// Failure while preparing, compiling, or transactionally publishing device artifacts.
#[derive(Debug)]
pub enum EmitError {
    Io(io::Error),
    Compilation(Box<dyn std::error::Error + Send + Sync>),
    UnsupportedKernel { kernel: String, reason: String },
    Preflight { reason: String },
    InvalidArtifactName { kernel: String, reason: String },
    DuplicateArtifactName { kernel: String },
    InvalidArtifactDestination { path: PathBuf, reason: String },
    InvalidFinalizedArtifact { path: PathBuf, reason: String },
    MissingStagedArtifact { path: PathBuf },
    StagingExhausted { output_dir: PathBuf },
    InvalidProducer { reason: String },
    Ownership { reason: String },
    ArtifactOwnedByOtherProducer { kernel: String },
    OutputDirectoryChanged { path: PathBuf },
    SubprocessPathBoundary { reason: String },
    BuildAttempt { reason: String },
    Transaction(Box<ArtifactTransactionError>),
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Compilation(error) => write!(f, "{error}"),
            Self::UnsupportedKernel { kernel, reason } => {
                write!(
                    f,
                    "unsupported kernel shape for AMDGPU LLVM IR MVP: {kernel}: {reason}"
                )
            }
            Self::Preflight { reason } => write!(f, "device artifact preflight failed: {reason}"),
            Self::InvalidArtifactName { kernel, reason } => {
                write!(f, "invalid kernel artifact name `{kernel}`: {reason}")
            }
            Self::DuplicateArtifactName { kernel } => {
                write!(f, "duplicate kernel artifact name `{kernel}`")
            }
            Self::InvalidArtifactDestination { path, reason } => {
                write!(
                    f,
                    "invalid kernel artifact destination {}: {reason}",
                    path.display()
                )
            }
            Self::InvalidFinalizedArtifact { path, reason } => {
                write!(f, "invalid finalized artifact {}: {reason}", path.display())
            }
            Self::MissingStagedArtifact { path } => {
                write!(
                    f,
                    "compiler did not produce staged artifact {}",
                    path.display()
                )
            }
            Self::StagingExhausted { output_dir } => {
                write!(
                    f,
                    "could not reserve an artifact staging directory in {}",
                    output_dir.display()
                )
            }
            Self::InvalidProducer { reason } => write!(f, "invalid artifact producer: {reason}"),
            Self::Ownership { reason } => {
                write!(f, "invalid non-authoritative ownership registry: {reason}")
            }
            Self::ArtifactOwnedByOtherProducer { kernel } => {
                write!(f, "artifact name {kernel} is owned by another producer")
            }
            Self::OutputDirectoryChanged { path } => {
                write!(
                    f,
                    "artifact output directory changed while pinned: {}",
                    path.display()
                )
            }
            Self::SubprocessPathBoundary { reason } => {
                write!(
                    f,
                    "cannot establish pinned subprocess path boundary: {reason}"
                )
            }
            Self::BuildAttempt { reason } => {
                write!(f, "invalid artifact build attempt: {reason}")
            }
            Self::Transaction(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Compilation(error) => Some(error.as_ref()),
            Self::Transaction(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for EmitError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

const LOCK_FILE: &str = ".fe2o3-artifacts.lock";
const OWNERSHIP_FILE: &str = ".fe2o3-owners-v1";
const RECOVERY_OWNERSHIP_FILE: &str = ".fe2o3-owners-v1.recovery";
const ATTEMPT_FILE: &str = ".fe2o3-attempts-v1";
const RECOVERY_ATTEMPT_FILE: &str = ".fe2o3-attempts-v1.recovery";
const STAGED_OWNERSHIP_FILE: &str = "owners-v1.next";
const STAGING_PREFIX: &str = ".fe2o3-stage-";
const OWNERSHIP_MAGIC: &[u8] = b"FE2O3-OWNERS-V1\0";
const MAX_ARTIFACT_NAME_BYTES: usize = 128;
const MAX_PRODUCER_SOURCE_BYTES: usize = 4096;
const MAX_PRODUCERS: usize = 1024;
const MAX_KERNELS_PER_PRODUCER: usize = 4096;
const MAX_TOTAL_OWNED_KERNELS: usize = 4096;
const MAX_OWNERSHIP_BYTES: usize = 1024 * 1024;
const MAX_STAGING_ATTEMPTS: u64 = 64;
// Three files for every owned and ownerless kernel, plus bounded staging/metadata headroom.
const MAX_OUTPUT_ENTRIES: usize = MAX_TOTAL_OWNED_KERNELS * 7;
const MAX_FINALIZED_LLVM_IR_BYTES: usize = 16 * 1024 * 1024;
const MAX_FINALIZED_HSACO_BYTES: usize = 4 * 1024 * 1024;

static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

/// Non-authoritative cleanup identity for one compiler producer.
///
/// Source paths are intentionally retained exactly as rustc reports them. Callers that need to
/// match an existing producer must supply the byte-for-byte same path spelling.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProducerIdentity {
    stable_source: String,
    crate_name: String,
}

impl ProducerIdentity {
    /// Builds a cleanup identity from rustc's crate name and local source path.
    pub fn from_codegen(crate_name: &str, local_source: Option<&Path>) -> Result<Self, EmitError> {
        validate_simple_name(crate_name, "crate name")?;
        let stable_source = match local_source {
            Some(path) => {
                let path = path.to_str().ok_or_else(|| EmitError::InvalidProducer {
                    reason: "local crate source path is not UTF-8".to_string(),
                })?;
                format!("path:{path}")
            }
            None => format!("crate:{crate_name}"),
        };
        if stable_source.len() > MAX_PRODUCER_SOURCE_BYTES {
            return Err(EmitError::InvalidProducer {
                reason: format!("stable source identity exceeds {MAX_PRODUCER_SOURCE_BYTES} bytes"),
            });
        }
        if stable_source.ends_with(':') || stable_source.as_bytes().contains(&0) {
            return Err(EmitError::InvalidProducer {
                reason: "stable source identity is empty or contains a NUL byte".to_string(),
            });
        }

        Ok(Self {
            stable_source,
            crate_name: crate_name.to_string(),
        })
    }

    #[cfg(test)]
    fn for_test(crate_name: &str, source: &str) -> Self {
        Self::from_codegen(crate_name, Some(Path::new(source))).unwrap()
    }
}

/// Test-only observation of a blocked `begin_build_attempt` lock acquisition.
///
/// This hook is coordination instrumentation, not artifact or launch authority. It is available
/// only when the `test-hooks` feature is explicitly enabled.
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
pub struct BeginBuildAttemptLockProbeV1 {
    inner: Arc<BeginBuildAttemptLockProbeInnerV1>,
}

#[cfg(feature = "test-hooks")]
struct BeginBuildAttemptLockProbeInnerV1 {
    output_dir: PathBuf,
    producer: ProducerIdentity,
    state: Mutex<BeginBuildAttemptLockProbeStateV1>,
    changed: Condvar,
}

#[cfg(feature = "test-hooks")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
enum BeginBuildAttemptLockProbeStateV1 {
    Installed,
    BeforeBlockingAcquire,
    Contended,
}

#[cfg(feature = "test-hooks")]
impl BeginBuildAttemptLockProbeInnerV1 {
    fn advance_to(&self, next: BeginBuildAttemptLockProbeStateV1) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if *state < next {
            *state = next;
            self.changed.notify_all();
        }
    }
}

#[cfg(feature = "test-hooks")]
impl BeginBuildAttemptLockProbeV1 {
    /// Blocks until the matching build attempt has observed the existing same-process lock and is
    /// immediately about to wait for its release.
    pub fn wait_until_contended(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while *state < BeginBuildAttemptLockProbeStateV1::Contended {
            state = self
                .inner
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

#[cfg(feature = "test-hooks")]
struct BeginBuildAttemptLockProbeRegistryV1 {
    probes: Mutex<Vec<Weak<BeginBuildAttemptLockProbeInnerV1>>>,
}

#[cfg(feature = "test-hooks")]
impl BeginBuildAttemptLockProbeRegistryV1 {
    fn global() -> &'static Self {
        static REGISTRY: OnceLock<BeginBuildAttemptLockProbeRegistryV1> = OnceLock::new();
        REGISTRY.get_or_init(|| BeginBuildAttemptLockProbeRegistryV1 {
            probes: Mutex::new(Vec::new()),
        })
    }

    fn observation(
        &self,
        output_dir: &Path,
        producer: &ProducerIdentity,
    ) -> BeginBuildAttemptLockObservationV1 {
        let mut registered = self
            .probes
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut matching = Vec::new();
        registered.retain(|probe| {
            let Some(probe) = probe.upgrade() else {
                return false;
            };
            if probe.output_dir == output_dir && probe.producer == *producer {
                matching.push(probe);
            }
            true
        });
        BeginBuildAttemptLockObservationV1 { matching }
    }
}

#[cfg(feature = "test-hooks")]
struct BeginBuildAttemptLockObservationV1 {
    matching: Vec<Arc<BeginBuildAttemptLockProbeInnerV1>>,
}

#[cfg(feature = "test-hooks")]
impl BeginBuildAttemptLockObservationV1 {
    fn advance_to(&self, state: BeginBuildAttemptLockProbeStateV1) {
        for probe in &self.matching {
            probe.advance_to(state);
        }
    }
}

/// Installs a test-only observer for one exact build attempt lock acquisition.
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
pub fn install_begin_build_attempt_lock_probe_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
) -> BeginBuildAttemptLockProbeV1 {
    let inner = Arc::new(BeginBuildAttemptLockProbeInnerV1 {
        output_dir: output_dir.to_path_buf(),
        producer: producer.clone(),
        state: Mutex::new(BeginBuildAttemptLockProbeStateV1::Installed),
        changed: Condvar::new(),
    });
    BeginBuildAttemptLockProbeRegistryV1::global()
        .probes
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(Arc::downgrade(&inner));
    BeginBuildAttemptLockProbeV1 { inner }
}

struct ArtifactProcessSpawnStateV1 {
    pid: u32,
    active_spawns: u64,
}

struct ArtifactProcessSpawnCoordinatorV1 {
    state: Mutex<ArtifactProcessSpawnStateV1>,
    idle: Condvar,
}

impl ArtifactProcessSpawnCoordinatorV1 {
    fn global() -> &'static Self {
        static COORDINATOR: OnceLock<ArtifactProcessSpawnCoordinatorV1> = OnceLock::new();
        COORDINATOR.get_or_init(|| Self {
            state: Mutex::new(ArtifactProcessSpawnStateV1 {
                pid: process::id(),
                active_spawns: 0,
            }),
            idle: Condvar::new(),
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, ArtifactProcessSpawnStateV1> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let pid = process::id();
        if state.pid != pid {
            state.pid = pid;
            state.active_spawns = 0;
        }
        state
    }

    fn begin_spawn(&'static self) -> ArtifactProcessSpawnLeaseV1 {
        let mut state = self.state();
        state.active_spawns = state
            .active_spawns
            .checked_add(1)
            .expect("concurrent artifact process spawn count overflowed");
        ArtifactProcessSpawnLeaseV1 { coordinator: self }
    }

    fn release_lock_descriptors(&self, release: impl FnOnce()) {
        let mut state = self.state();
        while state.active_spawns != 0 {
            state = self
                .idle
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        // Keep the state lock held while descriptors close so a new child cannot inherit them.
        release();
    }
}

struct ArtifactProcessSpawnLeaseV1 {
    coordinator: &'static ArtifactProcessSpawnCoordinatorV1,
}

impl Drop for ArtifactProcessSpawnLeaseV1 {
    fn drop(&mut self) {
        let mut state = self.coordinator.state();
        state.active_spawns = state
            .active_spawns
            .checked_sub(1)
            .expect("artifact process spawn lease underflowed");
        if state.active_spawns == 0 {
            self.coordinator.idle.notify_all();
        }
    }
}

/// Runs one process creation operation without exposing inherited artifact-lock aliases.
///
/// On Linux, a child temporarily retains the parent's `CLOEXEC` OFD and `flock` descriptors
/// between `fork` and `exec`. Every process creation in a process that uses this crate's artifact
/// transactions must pass its `Command::spawn` operation through this function. Artifact lock
/// release then waits for all coordinated children to exec or fail, ensuring that a child can
/// never become the sole owner of an inherited lock alias. Lock acquisition remains nonblocking
/// and reports only genuine lock contention.
pub fn with_artifact_process_spawn_v1<T, E>(spawn: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let _spawn = ArtifactProcessSpawnCoordinatorV1::global().begin_spawn();
    spawn()
}

/// Compatibility entry point for test fixtures that predate production spawn coordination.
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
pub fn with_test_artifact_fork_exec_barrier_v1<T, E>(
    spawn: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    with_artifact_process_spawn_v1(spawn)
}

/// Starts or resumes the durable artifact generation for one rustc invocation.
///
/// A new generation is recorded before this function invalidates the producer's prior owned
/// artifacts. The generation becomes backend-authorized only after invalidation and ownership
/// reconciliation are durable. Repeating this call for the same source, invocation, and session is
/// idempotent while the attempt is active; a distinct invocation fingerprint supersedes it.
/// Completed and failed invocations in the same session are terminal.
pub fn begin_build_attempt(
    output_dir: &Path,
    producer: &ProducerIdentity,
    invocation: BuildInvocation,
    session: BuildSession,
) -> Result<BuildAttempt, EmitError> {
    if session == BuildSession::DIRECT {
        return Err(build_attempt_error(
            "the all-zero build session is reserved for direct compiler invocations",
        ));
    }
    if invocation == BuildInvocation::DIRECT {
        return Err(build_attempt_error(
            "the all-zero build invocation is reserved for direct compiler invocations",
        ));
    }

    let output = PinnedOutput::open(output_dir)?;
    #[cfg(feature = "test-hooks")]
    let lock_observation =
        BeginBuildAttemptLockProbeRegistryV1::global().observation(output_dir, producer);
    #[cfg(feature = "test-hooks")]
    lock_observation.advance_to(BeginBuildAttemptLockProbeStateV1::BeforeBlockingAcquire);
    #[cfg(feature = "test-hooks")]
    let _lock = output.lock_for_build_attempt(&lock_observation)?;
    #[cfg(not(feature = "test-hooks"))]
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let mut attempts = read_attempt_registry(&output)?;
    cleanup_abandoned_staging(&output)?;
    if let Some(record) = attempts.record(&producer.stable_source)
        && record.session == session
        && record.crate_name != producer.crate_name
    {
        return Err(build_attempt_error(
            "one source has conflicting crate names in the same build session",
        ));
    }
    let outcome = attempts
        .start_or_resume(
            &producer.stable_source,
            &producer.crate_name,
            invocation,
            session,
        )
        .map_err(build_attempt_error)?;
    let attempt = match outcome {
        StartAttemptOutcome::ReuseBuilding(attempt) => return Ok(attempt),
        StartAttemptOutcome::New(attempt) => {
            commit_attempt_registry_direct(&output, &attempts)?;
            attempt
        }
        StartAttemptOutcome::ResumeInvalidating(attempt) => attempt,
    };

    invalidate_producer_ownership(&output, producer, &mut NoFaults)?;
    attempts
        .transition_building(&producer.stable_source, attempt)
        .map_err(build_attempt_error)?;
    commit_attempt_registry_direct(&output, &attempts)?;
    Ok(attempt)
}

/// Marks an exact build attempt failed before invalidating all artifacts still owned by it.
///
/// A stale or superseded token is rejected without mutating the current generation. Revocation is
/// transactional: if the initial durable claim reports an error, this function does not proceed to
/// the failed state or artifact invalidation. A replayable claim may still be recovered later.
pub fn fail_build_attempt(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), EmitError> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(build_attempt_error(
            "the direct compiler token is not valid for cargo-managed failure",
        ));
    }
    let output = PinnedOutput::open(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    claim_attempt_for_termination_locked(&output, producer, attempt)?;
    fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults)
}

/// Finishes an exact build attempt after at least one authorized backend publication succeeded.
///
/// Finishing durably marks the attempt completed; the terminal record prevents an older direct
/// frontend from publishing afterward. Ownership remains until the next generation or an explicit
/// failure invalidates it. A build with no observed backend fails closed and invalidates the
/// producer's artifacts. If the initial durable claim for that failure reports an error, this
/// function does not proceed to the failed state or artifact invalidation; a replayable claim may
/// still be recovered later.
pub fn finish_build_attempt(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), EmitError> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(build_attempt_error(
            "the direct compiler token is not valid for cargo-managed completion",
        ));
    }
    let output = PinnedOutput::open(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;

    let mut attempts = read_attempt_registry(&output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)?;
    if record.crate_name != producer.crate_name {
        return Err(build_attempt_error(
            "build attempt crate name does not match the producer",
        ));
    }
    if record.phase == AttemptPhase::Completed {
        return if record
            .backend_receipt
            .is_some_and(attempt::BackendReceiptV1::is_artifact_completion)
        {
            Ok(())
        } else {
            Err(build_attempt_error(
                "completed observation-only attempt has no authorized backend publication",
            ))
        };
    }
    if (record.phase == AttemptPhase::Building || record.phase == AttemptPhase::BackendClaimed)
        && !record
            .backend_receipt
            .is_some_and(attempt::BackendReceiptV1::is_artifact_completion)
    {
        let primary = build_attempt_error("build completed without an authorized device backend");
        claim_attempt_for_termination_locked(&output, producer, attempt)?;
        return match fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults) {
            Ok(()) => Err(primary),
            Err(secondary) => Err(combine_attempt_errors(primary, secondary, &output)),
        };
    }
    attempts
        .mark_completed(&producer.stable_source, attempt)
        .map_err(build_attempt_error)?;
    commit_attempt_registry_direct(&output, &attempts)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProducerOwnership {
    crate_name: String,
    kernels: BTreeSet<String>,
}

// Cleanup bookkeeping only. It is neither launch authority nor evidence that an artifact is valid.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct OwnershipRegistry {
    producers: BTreeMap<String, ProducerOwnership>,
}

impl OwnershipRegistry {
    fn owned_by(&self, producer: &ProducerIdentity) -> BTreeSet<String> {
        self.producers
            .get(&producer.stable_source)
            .map(|ownership| ownership.kernels.clone())
            .unwrap_or_default()
    }

    fn owner_of<'a>(&'a self, kernel: &str) -> Option<&'a str> {
        self.producers.iter().find_map(|(source, ownership)| {
            ownership
                .kernels
                .contains(kernel)
                .then_some(source.as_str())
        })
    }

    fn set_owned(&mut self, producer: &ProducerIdentity, kernels: BTreeSet<String>) {
        if kernels.is_empty() {
            self.producers.remove(&producer.stable_source);
        } else {
            self.producers.insert(
                producer.stable_source.clone(),
                ProducerOwnership {
                    crate_name: producer.crate_name.clone(),
                    kernels,
                },
            );
        }
    }

    fn encode(&self) -> Result<Vec<u8>, EmitError> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OWNERSHIP_MAGIC);
        push_u32(&mut bytes, self.producers.len())?;
        for (source, ownership) in &self.producers {
            push_text(&mut bytes, source)?;
            push_text(&mut bytes, &ownership.crate_name)?;
            push_u32(&mut bytes, ownership.kernels.len())?;
            for kernel in &ownership.kernels {
                push_text(&mut bytes, kernel)?;
            }
        }
        if bytes.len() > MAX_OWNERSHIP_BYTES {
            return Err(EmitError::Ownership {
                reason: "canonical ownership registry exceeds its byte bound".to_string(),
            });
        }
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, EmitError> {
        if bytes.len() > MAX_OWNERSHIP_BYTES {
            return Err(ownership_error("ownership registry exceeds its byte bound"));
        }
        let mut decoder = Decoder::new(bytes);
        if decoder.take(OWNERSHIP_MAGIC.len())? != OWNERSHIP_MAGIC {
            return Err(ownership_error("bad ownership registry magic"));
        }
        let producer_count = decoder.u32()? as usize;
        if producer_count > MAX_PRODUCERS {
            return Err(ownership_error("too many ownership producers"));
        }

        let mut producers = BTreeMap::new();
        let mut total_kernels = 0usize;
        for _ in 0..producer_count {
            let source = decoder.text(MAX_PRODUCER_SOURCE_BYTES)?;
            validate_stable_source(&source)?;
            let crate_name = decoder.text(MAX_ARTIFACT_NAME_BYTES)?;
            validate_simple_name(&crate_name, "owned crate name")?;
            let kernel_count = decoder.u32()? as usize;
            if kernel_count > MAX_KERNELS_PER_PRODUCER {
                return Err(ownership_error("too many kernels for one producer"));
            }
            total_kernels = total_kernels
                .checked_add(kernel_count)
                .ok_or_else(|| ownership_error("owned kernel count overflow"))?;
            if total_kernels > MAX_TOTAL_OWNED_KERNELS {
                return Err(ownership_error("too many kernels in ownership registry"));
            }

            let mut kernels = BTreeSet::new();
            for _ in 0..kernel_count {
                let kernel = decoder.text(MAX_ARTIFACT_NAME_BYTES)?;
                validate_artifact_name(&kernel)?;
                if !kernels.insert(kernel) {
                    return Err(ownership_error("duplicate owned kernel name"));
                }
            }
            if producers
                .insert(
                    source,
                    ProducerOwnership {
                        crate_name,
                        kernels,
                    },
                )
                .is_some()
            {
                return Err(ownership_error("duplicate producer identity"));
            }
        }
        if !decoder.is_finished() {
            return Err(ownership_error("trailing ownership registry bytes"));
        }

        let registry = Self { producers };
        registry.validate()?;
        if registry.encode()? != bytes {
            return Err(ownership_error("ownership registry is not canonical"));
        }
        Ok(registry)
    }

    fn validate(&self) -> Result<(), EmitError> {
        if self.producers.len() > MAX_PRODUCERS {
            return Err(ownership_error("too many ownership producers"));
        }
        let mut all_kernels = BTreeSet::new();
        for (source, ownership) in &self.producers {
            validate_stable_source(source)?;
            validate_simple_name(&ownership.crate_name, "owned crate name")?;
            if ownership.kernels.is_empty() || ownership.kernels.len() > MAX_KERNELS_PER_PRODUCER {
                return Err(ownership_error("invalid per-producer kernel count"));
            }
            for kernel in &ownership.kernels {
                validate_artifact_name(kernel)?;
                if !all_kernels.insert(kernel) {
                    return Err(ownership_error(
                        "one artifact name is owned by multiple producers",
                    ));
                }
            }
        }
        if all_kernels.len() > MAX_TOTAL_OWNED_KERNELS {
            return Err(ownership_error("too many kernels in ownership registry"));
        }
        Ok(())
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], EmitError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| ownership_error("ownership offset overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| ownership_error("truncated ownership registry"))?;
        self.offset = end;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, EmitError> {
        let bytes: [u8; 2] = self.take(2)?.try_into().unwrap();
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, EmitError> {
        let bytes: [u8; 4] = self.take(4)?.try_into().unwrap();
        Ok(u32::from_le_bytes(bytes))
    }

    fn text(&mut self, maximum: usize) -> Result<String, EmitError> {
        let length = self.u16()? as usize;
        if length == 0 || length > maximum {
            return Err(ownership_error("invalid ownership text length"));
        }
        String::from_utf8(self.take(length)?.to_vec())
            .map_err(|_| ownership_error("ownership text is not UTF-8"))
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), EmitError> {
    let value = u32::try_from(value).map_err(|_| ownership_error("ownership count overflow"))?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn push_text(bytes: &mut Vec<u8>, text: &str) -> Result<(), EmitError> {
    let length =
        u16::try_from(text.len()).map_err(|_| ownership_error("ownership text length overflow"))?;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

fn validate_stable_source(source: &str) -> Result<(), EmitError> {
    if source.len() > MAX_PRODUCER_SOURCE_BYTES
        || !(source.starts_with("path:") || source.starts_with("crate:"))
        || source.ends_with(':')
        || source.as_bytes().contains(&0)
    {
        return Err(ownership_error("invalid stable producer source"));
    }
    Ok(())
}

fn ownership_error(reason: impl Into<String>) -> EmitError {
    EmitError::Ownership {
        reason: reason.into(),
    }
}

/// Last durable publication milestone reached by a failed transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationState {
    /// No final artifact rename started.
    NotStarted { total_final_renames: usize },
    /// Some, but not all, final artifact renames completed before rollback.
    Partial {
        completed_final_renames: usize,
        total_final_renames: usize,
    },
    /// All final artifacts were renamed, but the ownership registry was not committed.
    FinalsPublished { final_renames: usize },
    /// The registry committed, but cleanup or final synchronization failed.
    CommittedWithCleanupFailure { final_renames: usize },
    /// The registry and final artifacts committed successfully.
    Committed { final_renames: usize },
}

impl PublicationState {
    /// Returns whether the ownership registry committed this generation.
    pub const fn is_committed(self) -> bool {
        matches!(
            self,
            Self::CommittedWithCleanupFailure { .. } | Self::Committed { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn spawn_test_process(command: &mut process::Command) -> io::Result<process::Child> {
        with_artifact_process_spawn_v1(|| command.spawn())
    }

    fn run_test_process(command: &mut process::Command) -> io::Result<process::ExitStatus> {
        let mut child = spawn_test_process(command)?;
        child.wait()
    }

    #[cfg(target_os = "linux")]
    fn close_child_descriptor_range(first: u32, last: u32) -> bool {
        if first > last {
            return true;
        }
        // SAFETY: this helper is called only in a fork child that immediately exits or pauses.
        unsafe { libc::syscall(libc::SYS_close_range, first, last, 0_u32) == 0 }
    }

    #[cfg(target_os = "linux")]
    fn close_unintended_child_descriptors(preserved: i32) -> bool {
        let Ok(preserved) = u32::try_from(preserved) else {
            return false;
        };
        if preserved < 3 {
            return close_child_descriptor_range(3, u32::MAX);
        }
        close_child_descriptor_range(3, preserved - 1)
            && close_child_descriptor_range(preserved + 1, u32::MAX)
    }

    #[cfg(target_os = "linux")]
    struct RawChildGuard(libc::pid_t);

    #[cfg(target_os = "linux")]
    impl Drop for RawChildGuard {
        fn drop(&mut self) {
            // SAFETY: the guard owns one unreaped child that deliberately pauses forever.
            unsafe {
                libc::kill(self.0, libc::SIGKILL);
                let mut status = 0;
                while libc::waitpid(self.0, &mut status, 0) < 0 {
                    if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                        break;
                    }
                }
            }
        }
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            enable_same_mount_namespace_artifact_path_guard_v1();
            loop {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "fe2o3-artifact-transaction-test-{}-{id}",
                    process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self { path },
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("failed to create test directory: {error}"),
                }
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[derive(Clone)]
    struct TestKernel {
        name: &'static str,
        generation: &'static str,
        valid: bool,
    }

    #[derive(Default)]
    struct Faults {
        fail_stage_create: bool,
        fail_stage_stat: bool,
        fail_artifact_rename_at: Option<usize>,
        fail_ownership_rename: bool,
        fail_invalidate_entry: Option<String>,
        fail_cleanup: bool,
        replace_output_after_commit: Option<(PathBuf, PathBuf)>,
    }

    impl TransactionHooks for Faults {
        fn before_stage_create(&mut self) -> io::Result<()> {
            if self.fail_stage_create {
                self.fail_stage_create = false;
                Err(io::Error::other("injected staging creation failure"))
            } else {
                Ok(())
            }
        }

        fn before_stage_stat(&mut self) -> io::Result<()> {
            if self.fail_stage_stat {
                self.fail_stage_stat = false;
                Err(io::Error::other("injected staging stat failure"))
            } else {
                Ok(())
            }
        }

        fn before_rename(&mut self, kind: RenameKind, completed: usize) -> io::Result<()> {
            match kind {
                RenameKind::Artifact if self.fail_artifact_rename_at == Some(completed) => {
                    self.fail_artifact_rename_at = None;
                    Err(io::Error::other("injected artifact rename failure"))
                }
                RenameKind::Ownership if self.fail_ownership_rename => {
                    self.fail_ownership_rename = false;
                    Err(io::Error::other("injected ownership rename failure"))
                }
                _ => Ok(()),
            }
        }

        fn before_invalidate(&mut self, entry: &str) -> io::Result<()> {
            if self.fail_invalidate_entry.as_deref() == Some(entry) {
                self.fail_invalidate_entry = None;
                Err(io::Error::other("injected invalidation failure"))
            } else {
                Ok(())
            }
        }

        fn before_stage_cleanup(&mut self) -> io::Result<()> {
            if self.fail_cleanup {
                self.fail_cleanup = false;
                Err(io::Error::other("injected staging cleanup failure"))
            } else {
                Ok(())
            }
        }

        fn after_registry_commit(&mut self) -> io::Result<()> {
            if let Some((output, relocated)) = self.replace_output_after_commit.take() {
                fs::rename(&output, relocated)?;
                fs::create_dir(output)?;
            }
            Ok(())
        }
    }

    struct ControlCommitFault {
        point: ControlCommitPoint,
    }

    impl ControlCommitHooks for ControlCommitFault {
        fn before(&mut self, point: ControlCommitPoint) -> io::Result<()> {
            if point == self.point {
                Err(io::Error::other(format!(
                    "injected control commit failure at {point:?}"
                )))
            } else {
                Ok(())
            }
        }
    }

    fn fake_compile(llvm_ir_path: &Path, hsaco_path: &Path) -> Result<(), EmitError> {
        let llvm_ir = fs::read_to_string(llvm_ir_path)?;
        fs::write(hsaco_path.with_extension("o"), format!("object:{llvm_ir}"))?;
        fs::write(hsaco_path, format!("hsaco:{llvm_ir}"))?;
        Ok(())
    }

    fn run(
        output: &Path,
        producer: &ProducerIdentity,
        kernels: &[TestKernel],
    ) -> Result<Vec<DeviceArtifact>, EmitError> {
        emit_artifact_transaction(
            output,
            producer,
            kernels,
            |kernel| kernel.name,
            |kernel| {
                if kernel.valid {
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                } else {
                    Err(EmitError::UnsupportedKernel {
                        kernel: kernel.name.to_string(),
                        reason: "injected preflight failure".to_string(),
                    })
                }
            },
            fake_compile,
        )
    }

    fn run_with_faults(
        output: &Path,
        producer: &ProducerIdentity,
        kernels: &[TestKernel],
        faults: &mut Faults,
        compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
    ) -> Result<Vec<DeviceArtifact>, EmitError> {
        emit_artifact_transaction_with_hooks(
            output,
            BackendRequest {
                producer,
                attempt: None,
            },
            kernels,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            compile,
            faults,
        )
    }

    fn one(name: &'static str, generation: &'static str) -> [TestKernel; 1] {
        [TestKernel {
            name,
            generation,
            valid: true,
        }]
    }

    fn read_owned(output: &Path, producer: &ProducerIdentity) -> BTreeSet<String> {
        let pinned = PinnedOutput::open(output).unwrap();
        let _lock = pinned.lock().unwrap();
        read_registry(&pinned).unwrap().owned_by(producer)
    }

    fn assert_generation(output: &Path, names: &[&str], generation: &str) {
        for name in names {
            assert_eq!(
                fs::read_to_string(output.join(format!("{name}.ll"))).unwrap(),
                format!("{generation}:{name}")
            );
            assert_eq!(
                fs::read_to_string(output.join(format!("{name}.o"))).unwrap(),
                format!("object:{generation}:{name}")
            );
            assert_eq!(
                fs::read_to_string(output.join(format!("{name}.hsaco"))).unwrap(),
                format!("hsaco:{generation}:{name}")
            );
        }
    }

    fn assert_snapshot(artifact: &DeviceArtifact, generation: &str, name: &str) {
        assert_eq!(artifact.kernel_name, name);
        assert_eq!(
            artifact.llvm_ir.bytes(),
            format!("{generation}:{name}").as_bytes()
        );
        assert_eq!(
            artifact.hsaco.bytes(),
            format!("hsaco:{generation}:{name}").as_bytes()
        );
    }

    #[test]
    fn snapshots_remain_bound_to_their_exact_generation_after_republish() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        let first = run(&output, &producer, &one("alpha", "first"))
            .unwrap()
            .remove(0);
        let second = run(&output, &producer, &one("alpha", "second"))
            .unwrap()
            .remove(0);

        assert_generation(&output, &["alpha"], "second");
        assert_snapshot(&first, "first", "alpha");
        assert_snapshot(&second, "second", "alpha");
    }

    #[test]
    fn snapshots_survive_final_path_replacement_without_reopening() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let artifact = run(&output, &producer, &one("alpha", "pinned"))
            .unwrap()
            .remove(0);

        let displaced_ir = output.join("alpha.ll.displaced");
        let displaced_hsaco = output.join("alpha.hsaco.displaced");
        fs::rename(output.join("alpha.ll"), &displaced_ir).unwrap();
        fs::rename(output.join("alpha.hsaco"), &displaced_hsaco).unwrap();
        fs::write(output.join("alpha.ll"), b"replacement-ir").unwrap();
        fs::write(output.join("alpha.hsaco"), b"replacement-hsaco").unwrap();

        assert_eq!(
            fs::read(artifact.llvm_ir.path()).unwrap(),
            b"replacement-ir"
        );
        assert_eq!(
            fs::read(artifact.hsaco.path()).unwrap(),
            b"replacement-hsaco"
        );
        assert_snapshot(&artifact, "pinned", "alpha");
    }

    #[test]
    fn invalid_snapshot_sizes_fail_before_publication() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = one("alpha", "invalid");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                fs::write(hsaco.with_extension("o"), fs::read(llvm_ir)?)?;
                fs::write(hsaco, b"")?;
                Ok(())
            },
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::InvalidFinalizedArtifact { .. })
        ));
        assert_eq!(
            transaction.publication(),
            PublicationState::NotStarted {
                total_final_renames: 3
            }
        );
        assert_absent(&output, &["alpha"]);
        assert_no_staging(&output);
    }

    fn assert_absent(output: &Path, names: &[&str]) {
        for name in names {
            for extension in ["ll", "o", "hsaco"] {
                assert!(!output.join(format!("{name}.{extension}")).exists());
            }
        }
    }

    fn assert_no_staging(output: &Path) {
        let staging = fs::read_dir(output)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| is_staging_name(entry.file_name().to_string_lossy().as_bytes()))
            .count();
        assert_eq!(staging, 0);
    }

    #[test]
    fn registry_is_bounded_canonical_and_non_authoritative_bookkeeping() {
        let producer = ProducerIdentity::for_test("producer", "/workspace/src/lib.rs");
        let mut registry = OwnershipRegistry::default();
        registry.set_owned(
            &producer,
            ["alpha".to_string(), "beta".to_string()]
                .into_iter()
                .collect(),
        );

        let encoded = registry.encode().unwrap();
        assert_eq!(OwnershipRegistry::decode(&encoded).unwrap(), registry);
        let mut trailing = encoded;
        trailing.push(0);
        assert!(OwnershipRegistry::decode(&trailing).is_err());
        assert!(matches!(
            ProducerIdentity::from_codegen(
                "producer",
                Some(Path::new(&"x".repeat(MAX_PRODUCER_SOURCE_BYTES + 1)))
            ),
            Err(EmitError::InvalidProducer { .. })
        ));

        let renamed = ProducerIdentity::for_test("renamed_crate", "/workspace/src/lib.rs");
        assert_eq!(producer.stable_source, renamed.stable_source);
        assert_ne!(producer.crate_name, renamed.crate_name);
    }

    #[test]
    fn rejects_unsafe_and_case_folded_duplicate_names_before_compile() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let outside = temp.path.join("escape.hsaco");
        fs::write(&outside, b"keep").unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let compile_calls = Cell::new(0usize);
        let unsafe_names = [
            TestKernel {
                name: "valid",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "../escape",
                generation: "new",
                valid: true,
            },
        ];

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &unsafe_names,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                compile_calls.set(compile_calls.get() + 1);
                fake_compile(llvm_ir, hsaco)
            },
        )
        .unwrap_err();
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::InvalidArtifactName { .. })
        ));
        assert_eq!(compile_calls.get(), 0);
        assert_eq!(fs::read(&outside).unwrap(), b"keep");
        assert_absent(&output, &["valid"]);

        let duplicate_names = [
            TestKernel {
                name: "Kernel",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "kernel",
                generation: "new",
                valid: true,
            },
        ];
        let error = run(&output, &producer, &duplicate_names).unwrap_err();
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::DuplicateArtifactName { .. })
        ));
        assert_no_staging(&output);
    }

    #[test]
    fn missing_compiler_output_fails_closed() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = one("alpha", "new");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |_llvm_ir, hsaco| {
                fs::write(hsaco, b"hsaco without object")?;
                Ok(())
            },
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::MissingStagedArtifact { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert_no_staging(&output);
    }

    #[test]
    fn output_symlink_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let temp = TestDirectory::new();
        let target = temp.path.join("target");
        let output = temp.path.join("output");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("unrelated"), b"keep").unwrap();
        symlink(&target, &output).unwrap();

        let error = run(
            &output,
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(error, EmitError::Io(_)));
        assert_eq!(fs::read(target.join("unrelated")).unwrap(), b"keep");
        assert_absent(&target, &["alpha"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_self_fd_output_stays_bound_to_retained_directory() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let retained_output = temp.path.join("retained-output");
        fs::create_dir(&output).unwrap();
        let retained = fs::File::open(&output).unwrap();
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", retained.as_raw_fd()));
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        run(&descriptor_path, &producer, &one("alpha", "first")).unwrap();
        fs::rename(&output, &retained_output).unwrap();
        fs::create_dir(&output).unwrap();
        fs::write(output.join("unrelated"), b"keep").unwrap();

        run(&descriptor_path, &producer, &one("alpha", "second")).unwrap();

        assert_generation(&retained_output, &["alpha"], "second");
        assert_eq!(fs::read(output.join("unrelated")).unwrap(), b"keep");
        assert_absent(&output, &["alpha"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_self_fd_output_requires_canonical_directory_descriptor() {
        let temp = TestDirectory::new();
        let file = temp.path.join("regular-file");
        fs::write(&file, b"not a directory").unwrap();
        let retained = fs::File::open(&file).unwrap();
        let non_directory = PathBuf::from(format!("/proc/self/fd/{}", retained.as_raw_fd()));

        assert!(matches!(
            PinnedOutput::open(Path::new("/proc/self/fd/01")),
            Err(EmitError::InvalidArtifactDestination { .. })
        ));
        assert!(matches!(
            PinnedOutput::open(&non_directory),
            Err(EmitError::InvalidArtifactDestination { .. })
        ));
    }

    #[test]
    fn parent_component_symlink_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let temp = TestDirectory::new();
        let real_parent = temp.path.join("real-parent");
        let linked_parent = temp.path.join("linked-parent");
        let target = real_parent.join("output");
        fs::create_dir(&real_parent).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(target.join("unrelated"), b"keep").unwrap();
        symlink(&real_parent, &linked_parent).unwrap();

        let error = run(
            &linked_parent.join("output"),
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(error, EmitError::Io(_)));
        assert_eq!(fs::read(target.join("unrelated")).unwrap(), b"keep");
        assert_absent(&target, &["alpha"]);
    }

    #[test]
    fn parent_directory_path_is_rejected_before_creating_any_prefix() {
        let temp = TestDirectory::new();
        let created_prefix = temp.path.join("must-not-exist");
        let output = created_prefix.join("..").join("output");

        let error = run(
            &output,
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            EmitError::InvalidArtifactDestination { .. }
        ));
        assert!(!created_prefix.exists());
    }

    #[test]
    fn relative_output_is_rejected_before_descriptor_or_path_guard_identity_can_diverge() {
        let relative = PathBuf::from(format!(
            "fe2o3-relative-output-must-not-exist-{}",
            std::process::id()
        ));
        assert!(!relative.exists());

        let Err(error) = PinnedOutput::open(&relative) else {
            panic!("relative artifact output was admitted");
        };

        assert!(matches!(
            error,
            EmitError::InvalidArtifactDestination { reason, .. }
                if reason.contains("must be absolute")
        ));
        assert!(!relative.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn filesystem_path_guard_directory_is_private_and_service_owned() {
        enable_same_mount_namespace_artifact_path_guard_v1();
        let service_uid = rustix::process::geteuid().as_raw();
        let (directory, path, whole_domain_lock) =
            open_linux_filesystem_path_guard_directory(service_uid).unwrap();
        let stat = fstat(&directory).unwrap();
        assert!(!whole_domain_lock);
        assert!(path.is_absolute());
        assert_eq!(FileType::from_raw_mode(stat.st_mode), FileType::Directory);
        assert_eq!(stat.st_uid, service_uid);
        assert_eq!(stat.st_mode & 0o7777, 0o700);

        let domain = FilesystemPathGuardDomain {
            directory,
            display_path: path.clone(),
            identity: ProcessLockIdentity::from_stat(&stat),
            lock_start: 0x5a,
            lock_length: 1,
            service_uid,
        };
        drop(acquire_linux_filesystem_path_guard(&domain, false).unwrap());
        let lock = fs::metadata(path.join(FILESYSTEM_PATH_GUARD_FILE)).unwrap();
        assert!(lock.file_type().is_file());
        assert_eq!(lock.uid(), service_uid);
        assert_eq!(lock.mode() & 0o7777, 0o600);
        assert_eq!(lock.nlink(), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn filesystem_path_guard_configuration_subprocess_helper() {
        let Some(action) = std::env::var_os("FE2O3_TEST_PATH_GUARD_ACTION") else {
            return;
        };
        let output = PathBuf::from(std::env::var_os("FE2O3_TEST_PATH_GUARD_OUTPUT").unwrap());
        let result = if action == "replace-after-admission" {
            let output = PinnedOutput::open(&output).unwrap();
            let configured =
                PathBuf::from(std::env::var_os(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV).unwrap());
            let displaced = configured.with_extension(format!("displaced-{}", process::id()));
            fs::rename(&configured, &displaced).unwrap();
            fs::create_dir(&configured).unwrap();
            fs::set_permissions(&configured, fs::Permissions::from_mode(0o700)).unwrap();
            output.lock().map(drop)
        } else if action == "replace-default-after-admission" {
            enable_same_mount_namespace_artifact_path_guard_v1();
            let output = PinnedOutput::open(&output).unwrap();
            let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
            let guard = runtime.join(FILESYSTEM_PATH_GUARD_RUNTIME_DIRECTORY);
            let displaced = guard.with_extension(format!("displaced-{}", process::id()));
            fs::rename(&guard, &displaced).unwrap();
            fs::create_dir(&guard).unwrap();
            fs::set_permissions(&guard, fs::Permissions::from_mode(0o700)).unwrap();
            output.lock().map(drop)
        } else if action == "configured-alias-contention" {
            let alias =
                PathBuf::from(std::env::var_os("FE2O3_TEST_PATH_GUARD_ALIAS_OUTPUT").unwrap());
            let first = PinnedOutput::open(&output).unwrap();
            let second = PinnedOutput::open(&alias).unwrap();
            let held = first.lock().unwrap();
            assert!(second.try_lock().unwrap().is_none());
            drop(held);
            second.try_lock().unwrap().unwrap();
            Ok(())
        } else if action == "configured-cross-process-holder" {
            let output = PinnedOutput::open(&output).unwrap();
            let held = output.lock().unwrap();
            let ready = PathBuf::from(std::env::var_os("FE2O3_TEST_PATH_GUARD_READY").unwrap());
            let release = PathBuf::from(std::env::var_os("FE2O3_TEST_PATH_GUARD_RELEASE").unwrap());
            fs::write(ready, b"ready").unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !release.exists() {
                assert!(
                    Instant::now() < deadline,
                    "configured path-guard holder was not released"
                );
                thread::sleep(Duration::from_millis(10));
            }
            drop(held);
            Ok(())
        } else if action == "configured-cross-process-contender" {
            let output = PinnedOutput::open(&output).unwrap();
            assert!(output.try_lock().unwrap().is_none());
            Ok(())
        } else if action == "same-namespace-accept" {
            enable_same_mount_namespace_artifact_path_guard_v1();
            PinnedOutput::open(&output).and_then(|output| output.lock().map(drop))
        } else {
            PinnedOutput::open(&output).and_then(|output| output.lock().map(drop))
        };
        match action.to_str().unwrap() {
            "accept"
            | "configured-alias-contention"
            | "configured-cross-process-holder"
            | "configured-cross-process-contender"
            | "same-namespace-accept" => result.unwrap(),
            "reject" | "replace-after-admission" | "replace-default-after-admission" => {
                assert!(matches!(
                    result,
                    Err(EmitError::InvalidArtifactDestination { .. })
                ))
            }
            other => panic!("unknown path-guard helper action {other}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn configured_path_guard_requires_a_preprovisioned_private_directory() {
        let temp = TestDirectory::new();
        let configured = temp.path.join("shared-path-guard");
        fs::create_dir(&configured).unwrap();
        fs::set_permissions(&configured, fs::Permissions::from_mode(0o700)).unwrap();
        let configured_metadata = fs::metadata(&configured).unwrap();
        let configured_identity = format!(
            "{:016x}:{:016x}",
            configured_metadata.dev(),
            configured_metadata.ino()
        );
        let tmp_metadata = fs::metadata("/tmp").unwrap();
        let tmp_identity = format!("{:016x}:{:016x}", tmp_metadata.dev(), tmp_metadata.ino());
        let helper = "tests::filesystem_path_guard_configuration_subprocess_helper";

        for (action, directory, identity, suffix, output_created) in [
            (
                "accept",
                Some(configured.as_path()),
                Some(configured_identity.as_str()),
                "accepted-output",
                true,
            ),
            (
                "reject",
                Some(configured.as_path()),
                Some("0000000000000000:0000000000000000"),
                "wrong-identity-output",
                false,
            ),
            (
                "reject",
                Some(Path::new("/tmp")),
                Some(tmp_identity.as_str()),
                "public-directory-output",
                false,
            ),
            (
                "reject",
                Some(configured.as_path()),
                None,
                "directory-only-output",
                false,
            ),
            (
                "reject",
                None,
                Some(configured_identity.as_str()),
                "identity-only-output",
                false,
            ),
            (
                "replace-after-admission",
                Some(configured.as_path()),
                Some(configured_identity.as_str()),
                "replaced-domain-output",
                true,
            ),
            ("reject", None, None, "unconfigured-output", false),
            (
                "same-namespace-accept",
                None,
                None,
                "same-namespace-output",
                true,
            ),
        ] {
            let output = temp.path.join(suffix);
            let mut command = process::Command::new(std::env::current_exe().unwrap());
            command
                .arg("--exact")
                .arg(helper)
                .arg("--nocapture")
                .env("FE2O3_TEST_PATH_GUARD_ACTION", action)
                .env("FE2O3_TEST_PATH_GUARD_OUTPUT", &output)
                .env("FE2O3_TEST_REQUIRE_EXPLICIT_PATH_GUARD", "1")
                .env_remove(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV)
                .env_remove(FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV);
            if let Some(directory) = directory {
                command.env(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV, directory);
            }
            if let Some(identity) = identity {
                command.env(FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV, identity);
            }
            let status = run_test_process(&mut command).unwrap();
            assert!(status.success(), "configured path-guard helper failed");
            assert_eq!(output.exists(), output_created);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn default_path_guard_replacement_after_admission_fails_closed() {
        let temp = TestDirectory::new();
        let runtime = temp.path.join("runtime");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let output = temp.path.join("default-replaced-domain-output");
        let mut command = process::Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("tests::filesystem_path_guard_configuration_subprocess_helper")
            .arg("--nocapture")
            .env(
                "FE2O3_TEST_PATH_GUARD_ACTION",
                "replace-default-after-admission",
            )
            .env("FE2O3_TEST_PATH_GUARD_OUTPUT", &output)
            .env("XDG_RUNTIME_DIR", &runtime)
            .env_remove(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV)
            .env_remove(FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV);
        let status = run_test_process(&mut command).unwrap();
        assert!(status.success(), "default path-guard helper failed");
        assert!(output.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn configured_path_guard_serializes_distinct_absolute_aliases() {
        let temp = TestDirectory::new();
        let configured = temp.path.join("shared-path-guard-alias-domain");
        fs::create_dir(&configured).unwrap();
        fs::set_permissions(&configured, fs::Permissions::from_mode(0o700)).unwrap();
        let metadata = fs::metadata(&configured).unwrap();
        let identity = format!("{:016x}:{:016x}", metadata.dev(), metadata.ino());
        let first = temp.path.join("first-output-alias");
        let second = temp.path.join("second-output-alias");
        let mut command = process::Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("tests::filesystem_path_guard_configuration_subprocess_helper")
            .arg("--nocapture")
            .env(
                "FE2O3_TEST_PATH_GUARD_ACTION",
                "configured-alias-contention",
            )
            .env("FE2O3_TEST_PATH_GUARD_OUTPUT", &first)
            .env("FE2O3_TEST_PATH_GUARD_ALIAS_OUTPUT", &second)
            .env(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV, &configured)
            .env(FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV, &identity);
        let status = run_test_process(&mut command).unwrap();
        assert!(
            status.success(),
            "configured alias contention helper failed"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn configured_path_guard_serializes_distinct_outputs_across_processes() {
        let temp = TestDirectory::new();
        let configured = temp.path.join("shared-cross-process-path-guard-domain");
        fs::create_dir(&configured).unwrap();
        fs::set_permissions(&configured, fs::Permissions::from_mode(0o700)).unwrap();
        let metadata = fs::metadata(&configured).unwrap();
        let identity = format!("{:016x}:{:016x}", metadata.dev(), metadata.ino());
        let first = temp.path.join("first-cross-process-output");
        let second = temp.path.join("second-cross-process-output");
        let ready = temp.path.join("cross-process-holder-ready");
        let release = temp.path.join("cross-process-holder-release");
        let helper = "tests::filesystem_path_guard_configuration_subprocess_helper";

        let mut holder_command = process::Command::new(std::env::current_exe().unwrap());
        holder_command
            .arg("--exact")
            .arg(helper)
            .arg("--nocapture")
            .env(
                "FE2O3_TEST_PATH_GUARD_ACTION",
                "configured-cross-process-holder",
            )
            .env("FE2O3_TEST_PATH_GUARD_OUTPUT", &first)
            .env("FE2O3_TEST_PATH_GUARD_READY", &ready)
            .env("FE2O3_TEST_PATH_GUARD_RELEASE", &release)
            .env(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV, &configured)
            .env(FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV, &identity);
        let mut holder = spawn_test_process(&mut holder_command).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            if let Some(status) = holder.try_wait().unwrap() {
                panic!("configured path-guard holder exited before readiness: {status}");
            }
            if Instant::now() >= deadline {
                holder.kill().unwrap();
                let _ = holder.wait();
                panic!("configured path-guard holder did not become ready");
            }
            thread::sleep(Duration::from_millis(10));
        }

        let mut contender_command = process::Command::new(std::env::current_exe().unwrap());
        contender_command
            .arg("--exact")
            .arg(helper)
            .arg("--nocapture")
            .env(
                "FE2O3_TEST_PATH_GUARD_ACTION",
                "configured-cross-process-contender",
            )
            .env("FE2O3_TEST_PATH_GUARD_OUTPUT", &second)
            .env(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV, &configured)
            .env(FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV, &identity);
        let contender = run_test_process(&mut contender_command).unwrap();
        fs::write(&release, b"release").unwrap();
        let holder = holder.wait().unwrap();
        assert!(
            contender.success(),
            "configured path-guard contender failed"
        );
        assert!(holder.success(), "configured path-guard holder failed");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn coordinated_fork_exec_does_not_leak_inherited_lock_aliases() {
        enable_same_mount_namespace_artifact_path_guard_v1();
        let temp = TestDirectory::new();
        let output = PinnedOutput::open(&temp.path.join("fork-exec-barrier-output")).unwrap();
        let contender = output.try_clone().unwrap();
        let held = output.lock().unwrap();
        let unrelated = PinnedOutput::open(&temp.path.join("unrelated-output")).unwrap();
        let (mut ready_parent, ready_child) = UnixStream::pair().unwrap();
        let (mut release_parent, release_child) = UnixStream::pair().unwrap();
        let spawn = thread::spawn(move || {
            let mut command = process::Command::new("/bin/sleep");
            command.arg("30");
            let ready_fd = ready_child.as_raw_fd();
            let release_fd = release_child.as_raw_fd();
            // SAFETY: the callback performs only async-signal-safe single-byte descriptor I/O.
            unsafe {
                command.pre_exec(move || {
                    let ready = [1_u8];
                    if libc::write(ready_fd, ready.as_ptr().cast(), ready.len()) != 1 {
                        return Err(io::Error::last_os_error());
                    }
                    let mut release = [0_u8];
                    loop {
                        let read =
                            libc::read(release_fd, release.as_mut_ptr().cast(), release.len());
                        if read == 1 {
                            return Ok(());
                        }
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::Interrupted {
                            return Err(error);
                        }
                    }
                });
            }
            with_artifact_process_spawn_v1(|| command.spawn()).unwrap()
        });
        let mut ready = [0_u8];
        ready_parent.read_exact(&mut ready).unwrap();
        assert_eq!(ready, [1]);

        let started = Instant::now();
        let unrelated_lock = unrelated
            .try_lock()
            .unwrap()
            .expect("an in-flight spawn synthesized Busy for an unrelated artifact");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "unrelated nonblocking acquisition waited on process creation"
        );

        // The child has inherited the holder's OFD aliases and is paused before exec. Releasing
        // the parent lock must wait, so the child never becomes the alias's sole owner.
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let lock_release = thread::spawn(move || {
            drop(held);
            dropped_tx.send(()).unwrap();
        });
        assert!(
            dropped_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "artifact lock descriptors closed before the fork/exec window ended"
        );
        let started = Instant::now();
        assert!(
            contender.try_lock().unwrap().is_none(),
            "nonblocking acquisition crossed the active fork/exec boundary"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "nonblocking acquisition waited on the fork/exec boundary"
        );
        release_parent.write_all(&[1]).unwrap();
        let mut child = spawn.join().unwrap();
        dropped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        lock_release.join().unwrap();
        drop(
            contender
                .lock()
                .expect("successful exec releases the child lock aliases"),
        );
        drop(unrelated_lock);
        assert!(child.try_wait().unwrap().is_none());
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nonblocking_lock_reports_immediate_busy_under_intentional_contention() {
        enable_same_mount_namespace_artifact_path_guard_v1();
        let temp = TestDirectory::new();
        let output = PinnedOutput::open(&temp.path.join("immediate-busy-output")).unwrap();
        let held = output.lock().unwrap();
        let contender = output.try_clone().unwrap();
        let (completed_tx, completed_rx) = mpsc::channel();
        let acquisition = thread::spawn(move || {
            completed_tx
                .send(contender.try_lock().unwrap().is_none())
                .unwrap();
        });

        let immediate_busy = completed_rx.recv_timeout(Duration::from_millis(40));
        drop(held);
        acquisition.join().unwrap();
        assert!(immediate_busy.unwrap_or_else(|_| {
            panic!("nonblocking lock acquisition waited instead of reporting Busy")
        }));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn lock_release_waits_for_every_active_process_spawn() {
        enable_same_mount_namespace_artifact_path_guard_v1();
        let temp = TestDirectory::new();
        let output = PinnedOutput::open(&temp.path.join("counted-spawn-output")).unwrap();
        let contender = output.try_clone().unwrap();
        let held = output.lock().unwrap();
        let coordinator = ArtifactProcessSpawnCoordinatorV1::global();
        let first = coordinator.begin_spawn();
        let second = coordinator.begin_spawn();
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let release = thread::spawn(move || {
            drop(held);
            dropped_tx.send(()).unwrap();
        });

        assert!(dropped_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(first);
        assert!(dropped_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(second);
        dropped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        release.join().unwrap();
        drop(contender.try_lock().unwrap().unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn paused_raw_child_cannot_retain_parent_lock_aliases() {
        enable_same_mount_namespace_artifact_path_guard_v1();
        let temp = TestDirectory::new();
        let output = PinnedOutput::open(&temp.path.join("raw-fork-alias-output")).unwrap();
        let contender = output.try_clone().unwrap();
        let held = output.lock().unwrap();
        let mut ready = [-1_i32; 2];
        // SAFETY: `ready` points to two writable descriptor slots.
        assert_eq!(
            unsafe { libc::pipe2(ready.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );

        let child = with_artifact_process_spawn_v1(|| {
            // SAFETY: the child closes the exact inherited lock aliases, reports readiness, and
            // pauses without returning to Rust. The parent reads readiness before releasing the
            // fork barrier, and reaps the child only after the barrier has been released.
            let child = unsafe { libc::fork() };
            assert!(child >= 0, "fork: {}", io::Error::last_os_error());
            if child == 0 {
                unsafe {
                    libc::close(ready[0]);
                    let byte = [1_u8];
                    if !close_unintended_child_descriptors(ready[1])
                        || libc::write(ready[1], byte.as_ptr().cast(), byte.len()) != 1
                    {
                        libc::_exit(127);
                    }
                    libc::close(ready[1]);
                    loop {
                        libc::pause();
                    }
                }
            }
            // SAFETY: the parent owns both pipe descriptors after fork.
            unsafe { libc::close(ready[1]) };
            let mut byte = [0_u8];
            loop {
                // SAFETY: the read descriptor and one-byte destination are valid.
                let read = unsafe { libc::read(ready[0], byte.as_mut_ptr().cast(), byte.len()) };
                if read == 1 {
                    break;
                }
                if read < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                panic!("raw child exited before releasing inherited lock aliases");
            }
            // SAFETY: readiness has been consumed and the parent no longer needs the pipe.
            unsafe { libc::close(ready[0]) };
            Ok::<_, io::Error>(child)
        })
        .unwrap();
        let _child = RawChildGuard(child);

        drop(held);
        drop(
            contender
                .lock()
                .expect("paused child retained an inherited artifact lock alias"),
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn retained_directory_clones_are_close_on_exec() {
        enable_same_mount_namespace_artifact_path_guard_v1();
        let temp = TestDirectory::new();
        let output = PinnedOutput::open(&temp.path.join("cloexec-output")).unwrap();
        let cloned = output.try_clone().unwrap();
        let descriptor_flags = |fd: &OwnedFd| {
            let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
            assert_ne!(flags, -1);
            flags
        };
        assert_ne!(descriptor_flags(&cloned.fd) & libc::FD_CLOEXEC, 0);
        assert_ne!(
            descriptor_flags(&cloned.path_guard.as_ref().unwrap().directory) & libc::FD_CLOEXEC,
            0,
        );
    }

    #[test]
    fn hardlinked_lock_is_rejected_without_mutating_the_other_inode() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let unrelated = temp.path.join("unrelated");
        fs::create_dir(&output).unwrap();
        fs::write(&unrelated, b"keep").unwrap();
        fs::set_permissions(&unrelated, fs::Permissions::from_mode(0o640)).unwrap();
        fs::hard_link(&unrelated, output.join(LOCK_FILE)).unwrap();

        let error = run(
            &output,
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            EmitError::InvalidArtifactDestination { .. }
        ));
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
        assert_eq!(
            fs::metadata(&unrelated).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::metadata(&unrelated).unwrap().nlink(), 2);
    }

    #[test]
    fn pinned_directory_substitution_fails_without_writing_replacement() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated = temp.path.join("relocated");
        fs::create_dir(&output).unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = one("alpha", "a");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| {
                fs::rename(&output, &relocated)?;
                fs::create_dir(&output)?;
                Ok(format!("{}:{}", kernel.generation, kernel.name))
            },
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert_absent(&relocated, &["alpha"]);
        assert_no_staging(&relocated);
    }

    #[test]
    fn pinned_parent_substitution_fails_without_writing_replacement() {
        let temp = TestDirectory::new();
        let parent = temp.path.join("parent");
        let output = parent.join("output");
        let relocated_parent = temp.path.join("relocated-parent");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&output).unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = one("alpha", "a");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| {
                fs::rename(&parent, &relocated_parent)?;
                fs::create_dir(&parent)?;
                fs::create_dir(parent.join("output"))?;
                Ok(format!("{}:{}", kernel.generation, kernel.name))
            },
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert_absent(&relocated_parent.join("output"), &["alpha"]);
        assert_no_staging(&relocated_parent.join("output"));
    }

    #[test]
    fn entire_collection_is_preflighted_before_compile_and_stale_outputs_are_invalidated() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let old = [
            TestKernel {
                name: "alpha",
                generation: "old",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "old",
                valid: true,
            },
        ];
        run(&output, &producer, &old).unwrap();
        let next = [
            TestKernel {
                name: "alpha",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "new",
                valid: false,
            },
        ];
        let compile_calls = Cell::new(0usize);

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &next,
            |kernel| kernel.name,
            |kernel| {
                if kernel.valid {
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                } else {
                    Err(EmitError::UnsupportedKernel {
                        kernel: kernel.name.to_string(),
                        reason: "injected preflight failure".to_string(),
                    })
                }
            },
            |llvm_ir, hsaco| {
                compile_calls.set(compile_calls.get() + 1);
                fake_compile(llvm_ir, hsaco)
            },
        )
        .unwrap_err();

        assert_eq!(compile_calls.get(), 0);
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::UnsupportedKernel { .. })
        ));
        assert_eq!(
            transaction.publication,
            PublicationState::NotStarted {
                total_final_renames: 6,
            }
        );
        assert_absent(&output, &["alpha", "beta"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn failure_before_kernel_discovery_invalidates_previous_outputs() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("alpha", "old")).unwrap();

        let error = emit_artifact_transaction_after_preflight(
            &output,
            &producer,
            || -> Result<Vec<TestKernel>, EmitError> {
                Err(EmitError::Preflight {
                    reason: "injected collection failure".to_string(),
                })
            },
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::Preflight { .. })
        ));
        assert_eq!(
            transaction.publication,
            PublicationState::NotStarted {
                total_final_renames: 0,
            }
        );
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn staging_creation_failure_repairs_and_persists_ownership() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("alpha", "old")).unwrap();
        let mut faults = Faults {
            fail_stage_create: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(transaction.primary.is_some());
        assert!(transaction.cleanup_failures.is_empty());
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert!(!output.join(RECOVERY_OWNERSHIP_FILE).exists());
        assert_no_staging(&output);
    }

    #[test]
    fn fully_absent_foreign_ownership_tombstone_is_pruned() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let old_producer = ProducerIdentity::for_test("old_producer", "/src/old.rs");
        let new_producer = ProducerIdentity::for_test("new_producer", "/src/new.rs");
        run(&output, &old_producer, &one("alpha", "old")).unwrap();
        for extension in ["ll", "o", "hsaco"] {
            fs::remove_file(output.join(format!("alpha.{extension}"))).unwrap();
        }

        run(&output, &new_producer, &one("alpha", "new")).unwrap();

        assert_generation(&output, &["alpha"], "new");
        assert!(read_owned(&output, &old_producer).is_empty());
        assert_eq!(
            read_owned(&output, &new_producer),
            ["alpha".to_string()].into()
        );
        assert_no_staging(&output);
    }

    #[test]
    fn staging_setup_cleanup_failure_is_reported_and_scavenged() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_stage_stat: true,
            fail_cleanup: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert!(fs::read_dir(&output).unwrap().any(|entry| {
            is_staging_name(entry.unwrap().file_name().to_string_lossy().as_bytes())
        }));

        run(&output, &producer, &one("alpha", "recovered")).unwrap();
        assert_generation(&output, &["alpha"], "recovered");
        assert_no_staging(&output);
    }

    #[test]
    fn compile_failure_cleans_all_staged_outputs_and_publishes_nothing() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = [
            TestKernel {
                name: "alpha",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "new",
                valid: true,
            },
        ];

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                if fs::read_to_string(llvm_ir)?.contains("beta") {
                    Err(io::Error::other("injected compile failure").into())
                } else {
                    fake_compile(llvm_ir, hsaco)
                }
            },
        )
        .unwrap_err();

        assert!(matches!(error, EmitError::Transaction(_)));
        assert_absent(&output, &["alpha", "beta"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn pre_registry_artifacts_are_replaced_and_adopted() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        fs::create_dir(&output).unwrap();
        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("alpha.{extension}")), b"legacy").unwrap();
        }
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        run(&output, &producer, &one("alpha", "new")).unwrap();

        assert_generation(&output, &["alpha"], "new");
        assert_eq!(read_owned(&output, &producer), ["alpha".to_string()].into());
        assert_no_staging(&output);
    }

    #[test]
    fn pre_registry_artifacts_are_invalidated_on_preflight_failure() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        fs::create_dir(&output).unwrap();
        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("alpha.{extension}")), b"stale").unwrap();
        }
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let invalid = [TestKernel {
            name: "alpha",
            generation: "new",
            valid: false,
        }];

        let error = run(&output, &producer, &invalid).unwrap_err();
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::UnsupportedKernel { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn pre_registry_removed_and_renamed_kernels_are_scavenged() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        fs::create_dir(&output).unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("removed.{extension}")), b"legacy").unwrap();
            fs::write(output.join(format!("old_name.{extension}")), b"legacy").unwrap();
        }
        run(&output, &producer, &one("new_name", "new")).unwrap();

        assert_absent(&output, &["removed", "old_name"]);
        assert_generation(&output, &["new_name"], "new");
        assert_eq!(
            read_owned(&output, &producer),
            ["new_name".to_string()].into()
        );

        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("zeroed.{extension}")), b"orphan").unwrap();
        }
        run(&output, &producer, &[]).unwrap();
        assert_absent(&output, &["new_name", "zeroed"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn abandoned_staging_is_scavenged_without_touching_noncanonical_files() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let abandoned = output.join(format!("{STAGING_PREFIX}999-1"));
        fs::create_dir_all(&abandoned).unwrap();
        fs::set_permissions(&abandoned, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(abandoned.join("alpha.ll"), b"partial").unwrap();
        fs::write(output.join("keep.txt"), b"keep").unwrap();
        fs::write(output.join("not-a-kernel.ll"), b"keep").unwrap();
        fs::write(output.join(".fe2o3-stage-not-reserved"), b"keep").unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        run(&output, &producer, &one("alpha", "new")).unwrap();

        assert!(!abandoned.exists());
        assert_eq!(fs::read(output.join("keep.txt")).unwrap(), b"keep");
        assert_eq!(fs::read(output.join("not-a-kernel.ll")).unwrap(), b"keep");
        assert_eq!(
            fs::read(output.join(".fe2o3-stage-not-reserved")).unwrap(),
            b"keep"
        );
        assert_generation(&output, &["alpha"], "new");
        assert_no_staging(&output);
    }

    #[test]
    fn concurrent_generations_are_serialized_and_never_mix() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let first_output = output.clone();
        let first_producer = producer.clone();
        let (first_entered_tx, first_entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first = thread::spawn(move || {
            let kernels = [
                TestKernel {
                    name: "alpha",
                    generation: "first",
                    valid: true,
                },
                TestKernel {
                    name: "beta",
                    generation: "first",
                    valid: true,
                },
            ];
            emit_artifact_transaction(
                &first_output,
                &first_producer,
                &kernels,
                |kernel| kernel.name,
                |kernel| {
                    if kernel.name == "alpha" {
                        first_entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                    }
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                },
                fake_compile,
            )
        });
        first_entered_rx.recv().unwrap();

        let second_output = output.clone();
        let second_producer = producer.clone();
        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            let kernels = [
                TestKernel {
                    name: "alpha",
                    generation: "second",
                    valid: true,
                },
                TestKernel {
                    name: "beta",
                    generation: "second",
                    valid: true,
                },
            ];
            emit_artifact_transaction(
                &second_output,
                &second_producer,
                &kernels,
                |kernel| kernel.name,
                |kernel| {
                    if kernel.name == "alpha" {
                        second_entered_tx.send(()).unwrap();
                    }
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                },
                fake_compile,
            )
        });

        assert!(
            second_entered_rx
                .recv_timeout(Duration::from_millis(150))
                .is_err()
        );
        release_tx.send(()).unwrap();
        let first_artifacts = first.join().unwrap().unwrap();
        second_entered_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let second_artifacts = second.join().unwrap().unwrap();

        assert_generation(&output, &["alpha", "beta"], "second");
        for artifact in &first_artifacts {
            assert_snapshot(artifact, "first", &artifact.kernel_name);
        }
        for artifact in &second_artifacts {
            assert_snapshot(artifact, "second", &artifact.kernel_name);
        }
        assert_no_staging(&output);
    }

    #[test]
    fn uncoordinated_fork_inherits_publication_lock_until_cloexec() {
        const ISOLATED_HELPER_ENV: &str = "FE2O3_TEST_UNCOORDINATED_OFD_FORK_HELPER";
        if std::env::var_os(ISOLATED_HELPER_ENV).is_none() {
            let mut command = process::Command::new(std::env::current_exe().unwrap());
            command
                .arg("--exact")
                .arg("tests::uncoordinated_fork_inherits_publication_lock_until_cloexec")
                .arg("--nocapture")
                .env(ISOLATED_HELPER_ENV, "1");
            assert!(run_test_process(&mut command).unwrap().success());
            return;
        }

        enable_same_mount_namespace_artifact_path_guard_v1();
        let temp = TestDirectory::new();
        let output = PinnedOutput::open(&temp.path.join("output")).unwrap();
        let lock = output.lock().unwrap();
        let (mut parent_control, child_control) = UnixStream::pair().unwrap();
        parent_control
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        child_control
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        let child = thread::spawn(move || {
            let mut command = process::Command::new("true");
            // Only async-signal-safe read/write syscalls run between fork and exec.
            unsafe {
                command.pre_exec(move || {
                    rustix::io::write(&child_control, &[1]).map_err(io::Error::from)?;
                    let mut go = [0];
                    let count =
                        rustix::io::read(&child_control, &mut go).map_err(io::Error::from)?;
                    if count != 1 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "parent did not release fork-to-exec child",
                        ));
                    }
                    Ok(())
                });
            }
            // This isolated helper intentionally bypasses production coordination to establish
            // the Linux inheritance behavior that the production boundary must contain.
            let mut child = command.spawn().unwrap();
            child.wait().unwrap()
        });

        let mut ready = [0];
        parent_control.read_exact(&mut ready).unwrap();
        drop(lock);
        assert!(
            output.try_lock().unwrap().is_none(),
            "forked child must retain its inherited OFD lock before exec"
        );
        parent_control.write_all(&[1]).unwrap();
        assert!(child.join().unwrap().success());
        drop(
            output
                .lock()
                .expect("CLOEXEC must release the forked child's OFD lock alias"),
        );
    }

    #[test]
    fn producers_reconcile_only_their_owned_sets_including_zero_kernels() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer_a = ProducerIdentity::for_test("producer_a", "/src/a.rs");
        let producer_b = ProducerIdentity::for_test("producer_b", "/src/b.rs");
        let barrier = Arc::new(Barrier::new(2));

        let a_output = output.clone();
        let a_producer = producer_a.clone();
        let a_barrier = Arc::clone(&barrier);
        let a = thread::spawn(move || {
            a_barrier.wait();
            run(&a_output, &a_producer, &one("alpha", "a"))
        });
        let b_output = output.clone();
        let b_producer = producer_b.clone();
        let b_barrier = Arc::clone(&barrier);
        let b = thread::spawn(move || {
            b_barrier.wait();
            run(&b_output, &b_producer, &one("beta", "b"))
        });
        a.join().unwrap().unwrap();
        b.join().unwrap().unwrap();

        assert_generation(&output, &["alpha"], "a");
        assert_generation(&output, &["beta"], "b");
        run(&output, &producer_a, &[]).unwrap();
        assert_absent(&output, &["alpha"]);
        assert_generation(&output, &["beta"], "b");
        assert!(read_owned(&output, &producer_a).is_empty());
        assert_eq!(
            read_owned(&output, &producer_b),
            ["beta".to_string()].into()
        );
    }

    #[test]
    fn renamed_kernel_removes_the_previous_owned_generation() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("old_name", "old")).unwrap();

        run(&output, &producer, &one("new_name", "new")).unwrap();

        assert_absent(&output, &["old_name"]);
        assert_generation(&output, &["new_name"], "new");
        assert_eq!(
            read_owned(&output, &producer),
            ["new_name".to_string()].into()
        );
    }

    #[test]
    fn partial_publish_is_rolled_back_and_reports_progress() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = [
            TestKernel {
                name: "alpha",
                generation: "old",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "old",
                valid: true,
            },
        ];
        run(&output, &producer, &kernels).unwrap();
        let next = kernels
            .iter()
            .cloned()
            .map(|mut kernel| {
                kernel.generation = "new";
                kernel
            })
            .collect::<Vec<_>>();
        let mut faults = Faults {
            fail_artifact_rename_at: Some(1),
            ..Faults::default()
        };

        let error =
            run_with_faults(&output, &producer, &next, &mut faults, fake_compile).unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert_eq!(
            transaction.publication,
            PublicationState::Partial {
                completed_final_renames: 1,
                total_final_renames: 6,
            }
        );
        assert!(transaction.primary.is_some());
        assert!(transaction.invalidation_failures.is_empty());
        assert_absent(&output, &["alpha", "beta"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn ownership_publish_failure_rolls_back_all_final_artifacts() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_ownership_rename: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert_eq!(
            transaction.publication,
            PublicationState::FinalsPublished { final_renames: 3 }
        );
        assert!(
            transaction
                .primary
                .as_ref()
                .unwrap()
                .to_string()
                .contains("injected ownership rename failure")
        );
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn committed_cleanup_failure_is_reported_without_rollback() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_cleanup: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "committed"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(transaction.primary.is_none());
        assert_eq!(
            transaction.publication,
            PublicationState::CommittedWithCleanupFailure { final_renames: 3 }
        );
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_generation(&output, &["alpha"], "committed");
        assert_eq!(read_owned(&output, &producer), ["alpha".to_string()].into());
        assert_no_staging(&output);
    }

    #[test]
    fn final_identity_failure_reports_committed_publication() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated = temp.path.join("relocated");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            replace_output_after_commit: Some((output.clone(), relocated.clone())),
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "committed"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_eq!(
            transaction.cleanup_failures[0].operation(),
            "persist failed build attempt"
        );
        assert_eq!(
            transaction.publication,
            PublicationState::Committed { final_renames: 3 }
        );
        assert_absent(&output, &["alpha"]);
        assert_generation(&relocated, &["alpha"], "committed");
        assert_eq!(
            read_owned(&relocated, &producer),
            ["alpha".to_string()].into()
        );
        assert_no_staging(&relocated);
    }

    #[test]
    fn final_identity_and_cleanup_failures_report_committed_with_cleanup_failure() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated = temp.path.join("relocated");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_cleanup: true,
            replace_output_after_commit: Some((output.clone(), relocated.clone())),
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "committed"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert_eq!(transaction.cleanup_failures.len(), 2);
        assert!(
            transaction
                .cleanup_failures
                .iter()
                .any(|failure| failure.operation() == "persist failed build attempt")
        );
        assert_eq!(
            transaction.publication,
            PublicationState::CommittedWithCleanupFailure { final_renames: 3 }
        );
        assert_absent(&output, &["alpha"]);
        assert_generation(&relocated, &["alpha"], "committed");
        assert_eq!(
            read_owned(&relocated, &producer),
            ["alpha".to_string()].into()
        );
        assert_no_staging(&relocated);
    }

    #[test]
    fn composite_error_preserves_primary_invalidation_and_cleanup_failures() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("alpha", "old")).unwrap();
        let mut faults = Faults {
            fail_invalidate_entry: Some("alpha.hsaco".to_string()),
            fail_cleanup: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            |_llvm_ir, _hsaco| Err(io::Error::other("injected compiler failure").into()),
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(
            transaction
                .primary
                .as_ref()
                .unwrap()
                .to_string()
                .contains("injected compiler failure")
        );
        assert_eq!(transaction.invalidation_failures.len(), 1);
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_eq!(
            transaction.publication,
            PublicationState::NotStarted {
                total_final_renames: 3,
            }
        );
        assert!(!output.join("alpha.ll").exists());
        assert!(!output.join("alpha.o").exists());
        assert!(output.join("alpha.hsaco").exists());
        assert_eq!(read_owned(&output, &producer), ["alpha".to_string()].into());
        assert_no_staging(&output);
    }

    #[test]
    fn private_staging_and_subprocess_boundary_publish_successfully() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let observed_mode = Arc::new(AtomicU64::new(0));
        let mode = Arc::clone(&observed_mode);

        emit_artifact_transaction(
            &output,
            &producer,
            &one("alpha", "a"),
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            move |llvm_ir, hsaco| {
                let staging = llvm_ir.parent().unwrap();
                mode.store(
                    fs::metadata(staging)?.permissions().mode() as u64 & 0o777,
                    Ordering::Relaxed,
                );
                fake_compile(llvm_ir, hsaco)
            },
        )
        .unwrap();

        assert_eq!(observed_mode.load(Ordering::Relaxed), 0o700);
        assert_generation(&output, &["alpha"], "a");
    }

    #[test]
    fn subprocess_fd_is_not_redirected_by_staging_name_substitution() {
        use std::os::unix::fs::symlink;

        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated_stage = temp.path.join("relocated-stage");
        let outside = temp.path.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"keep").unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &one("alpha", "a"),
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                let stage_entry = fs::read_dir(&output)?
                    .filter_map(Result::ok)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".fe2o3-stage-")
                    })
                    .expect("staging entry must exist")
                    .path();
                fs::rename(&stage_entry, &relocated_stage)?;
                symlink(&outside, &stage_entry)?;

                assert_eq!(
                    fs::canonicalize(llvm_ir.parent().unwrap())?,
                    fs::canonicalize(&relocated_stage)?
                );
                let object = hsaco.with_extension("o");
                let mut command = process::Command::new("sh");
                command
                    .args([
                        "-c",
                        "ir=$(cat \"$1\") || exit; printf 'object:%s' \"$ir\" > \"$2\"; printf 'hsaco:%s' \"$ir\" > \"$3\"",
                        "sh",
                    ])
                    .arg(llvm_ir)
                    .arg(&object)
                    .arg(hsaco);
                let status = run_test_process(&mut command)?;
                if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other(format!("test subprocess failed: {status}")).into())
                }
            },
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(transaction.primary.is_none());
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_eq!(
            transaction.publication,
            PublicationState::CommittedWithCleanupFailure { final_renames: 3 }
        );
        assert_generation(&output, &["alpha"], "a");
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"keep");
        assert_absent(&outside, &["alpha"]);
        assert!(
            fs::symlink_metadata(
                fs::read_dir(&output)
                    .unwrap()
                    .filter_map(Result::ok)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".fe2o3-stage-")
                    })
                    .unwrap()
                    .path()
            )
            .unwrap()
            .file_type()
            .is_symlink()
        );
        assert_eq!(fs::read_dir(&relocated_stage).unwrap().count(), 0);
    }

    #[test]
    fn interrupted_control_commits_never_reopen_a_consumed_backend_claim() {
        let points = [
            ControlCommitPoint::CreateRecovery,
            ControlCommitPoint::WriteRecovery,
            ControlCommitPoint::SyncRecovery,
            ControlCommitPoint::SyncRecoveryName,
            ControlCommitPoint::RenameRecovery,
            ControlCommitPoint::SyncFinalName,
        ];

        for point in points {
            let temp = TestDirectory::new();
            let output_path = temp.path.join("output");
            let output = PinnedOutput::open(&output_path).unwrap();
            let _lock = output.lock().unwrap();
            let session = BuildSession::from_bytes([0x31; 16]);
            let invocation = BuildInvocation::from_bytes([0x42; 32]);
            let mut registry = AttemptRegistry::default();
            let attempt = match registry
                .start_or_resume("path:/src/kernel.rs", "kernel", invocation, session)
                .unwrap()
            {
                StartAttemptOutcome::New(attempt) => attempt,
                outcome => panic!("unexpected attempt start: {outcome:?}"),
            };
            registry
                .transition_building("path:/src/kernel.rs", attempt)
                .unwrap();
            commit_attempt_registry_direct(&output, &registry).unwrap();

            registry
                .claim_backend("path:/src/kernel.rs", attempt)
                .unwrap();
            let bytes = registry.encode().unwrap();
            let error = commit_control_file_direct_with_hooks(
                &output,
                ATTEMPT_FILE,
                RECOVERY_ATTEMPT_FILE,
                Some(&bytes),
                &mut ControlCommitFault { point },
            )
            .unwrap_err();
            assert!(matches!(error, EmitError::Io(_)), "{point:?}: {error}");

            if point == ControlCommitPoint::CreateRecovery {
                assert!(!output_path.join(RECOVERY_ATTEMPT_FILE).exists());
                let current = read_attempt_registry(&output).unwrap();
                current
                    .authorize_backend("path:/src/kernel.rs", attempt)
                    .unwrap();
                continue;
            }

            match read_attempt_registry(&output) {
                Ok(current) => assert_eq!(
                    current.authorize_backend("path:/src/kernel.rs", attempt),
                    Err(AttemptCodecError::BackendAlreadySeen),
                    "{point:?}"
                ),
                Err(EmitError::BuildAttempt { .. }) => {
                    assert!(
                        output_path.join(RECOVERY_ATTEMPT_FILE).exists(),
                        "{point:?}"
                    );
                }
                Err(error) => panic!("unexpected recovery error at {point:?}: {error}"),
            }
        }
    }

    #[test]
    fn termination_claim_failure_never_reports_or_performs_revocation() {
        let points = [
            ControlCommitPoint::CreateRecovery,
            ControlCommitPoint::WriteRecovery,
            ControlCommitPoint::SyncRecovery,
            ControlCommitPoint::SyncRecoveryName,
            ControlCommitPoint::RenameRecovery,
            ControlCommitPoint::SyncFinalName,
        ];

        for point in points {
            let temp = TestDirectory::new();
            let output_path = temp.path.join("output");
            let output = PinnedOutput::open(&output_path).unwrap();
            let _lock = output.lock().unwrap();
            let producer = ProducerIdentity::for_test("kernel", "/src/kernel.rs");
            let session = BuildSession::from_bytes([0x71; 16]);
            let invocation = BuildInvocation::from_bytes([0x72; 32]);
            let mut registry = AttemptRegistry::default();
            let attempt = match registry
                .start_or_resume(
                    &producer.stable_source,
                    &producer.crate_name,
                    invocation,
                    session,
                )
                .unwrap()
            {
                StartAttemptOutcome::New(attempt) => attempt,
                outcome => panic!("unexpected attempt start: {outcome:?}"),
            };
            registry
                .transition_building(&producer.stable_source, attempt)
                .unwrap();
            commit_attempt_registry_direct(&output, &registry).unwrap();

            claim_attempt_for_termination_locked_with_hooks(
                &output,
                &producer,
                attempt,
                &mut ControlCommitFault { point },
            )
            .unwrap_err();

            if point == ControlCommitPoint::CreateRecovery {
                assert!(!output_path.join(RECOVERY_ATTEMPT_FILE).exists());
                read_attempt_registry(&output)
                    .unwrap()
                    .authorize_backend(&producer.stable_source, attempt)
                    .unwrap();
                continue;
            }
            match read_attempt_registry(&output) {
                Ok(current) => assert_eq!(
                    current.authorize_backend(&producer.stable_source, attempt),
                    Err(AttemptCodecError::BackendAlreadySeen),
                    "{point:?}"
                ),
                Err(EmitError::BuildAttempt { .. }) => assert!(
                    output_path.join(RECOVERY_ATTEMPT_FILE).exists(),
                    "{point:?}"
                ),
                Err(error) => panic!("unexpected recovery error at {point:?}: {error}"),
            }
        }
    }

    #[test]
    fn interrupted_failure_commit_leaves_the_prior_backend_claim_closed() {
        let points = [
            ControlCommitPoint::CreateRecovery,
            ControlCommitPoint::WriteRecovery,
            ControlCommitPoint::SyncRecovery,
            ControlCommitPoint::SyncRecoveryName,
            ControlCommitPoint::RenameRecovery,
            ControlCommitPoint::SyncFinalName,
        ];

        for point in points {
            let temp = TestDirectory::new();
            let output_path = temp.path.join("output");
            let output = PinnedOutput::open(&output_path).unwrap();
            let _lock = output.lock().unwrap();
            let session = BuildSession::from_bytes([0x51; 16]);
            let invocation = BuildInvocation::from_bytes([0x62; 32]);
            let mut registry = AttemptRegistry::default();
            let attempt = match registry
                .start_or_resume("path:/src/kernel.rs", "kernel", invocation, session)
                .unwrap()
            {
                StartAttemptOutcome::New(attempt) => attempt,
                outcome => panic!("unexpected attempt start: {outcome:?}"),
            };
            registry
                .transition_building("path:/src/kernel.rs", attempt)
                .unwrap();
            registry
                .claim_backend("path:/src/kernel.rs", attempt)
                .unwrap();
            commit_attempt_registry_direct(&output, &registry).unwrap();

            registry
                .mark_failed("path:/src/kernel.rs", attempt)
                .unwrap();
            let bytes = registry.encode().unwrap();
            commit_control_file_direct_with_hooks(
                &output,
                ATTEMPT_FILE,
                RECOVERY_ATTEMPT_FILE,
                Some(&bytes),
                &mut ControlCommitFault { point },
            )
            .unwrap_err();

            match read_attempt_registry(&output) {
                Ok(current) => assert!(
                    current
                        .authorize_backend("path:/src/kernel.rs", attempt)
                        .is_err(),
                    "{point:?}"
                ),
                Err(EmitError::BuildAttempt { .. }) => {
                    assert!(
                        output_path.join(RECOVERY_ATTEMPT_FILE).exists(),
                        "{point:?}"
                    );
                }
                Err(error) => panic!("unexpected recovery error at {point:?}: {error}"),
            }
        }
    }
}

/// One structured filesystem failure recorded while reconciling a transaction.
#[derive(Debug)]
pub struct FilesystemFailure {
    pub(crate) operation: &'static str,
    pub(crate) entry: String,
    pub(crate) error: io::Error,
}

impl FilesystemFailure {
    /// Operation that failed.
    pub const fn operation(&self) -> &'static str {
        self.operation
    }

    /// Managed-directory entry or diagnostic path associated with the failure.
    pub fn entry(&self) -> &str {
        &self.entry
    }

    /// Underlying operating-system error.
    pub const fn error(&self) -> &io::Error {
        &self.error
    }
}

impl fmt::Display for FilesystemFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} entry {} failed: {}",
            self.operation, self.entry, self.error
        )
    }
}

impl std::error::Error for FilesystemFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Structured failure report for one artifact transaction.
#[derive(Debug)]
pub struct ArtifactTransactionError {
    pub(crate) primary: Option<Box<EmitError>>,
    pub(crate) cleanup_failures: Vec<FilesystemFailure>,
    pub(crate) invalidation_failures: Vec<FilesystemFailure>,
    pub(crate) publication: PublicationState,
}

impl ArtifactTransactionError {
    /// Primary preparation, compilation, publication, or synchronization failure, if any.
    pub fn primary(&self) -> Option<&EmitError> {
        self.primary.as_deref()
    }

    /// Failures encountered while removing staging or recovery state.
    pub fn cleanup_failures(&self) -> &[FilesystemFailure] {
        &self.cleanup_failures
    }

    /// Failures encountered while invalidating final artifacts.
    pub fn invalidation_failures(&self) -> &[FilesystemFailure] {
        &self.invalidation_failures
    }

    /// Last durable publication milestone reached by the transaction.
    pub const fn publication(&self) -> PublicationState {
        self.publication
    }
}

impl fmt::Display for ArtifactTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.primary {
            Some(primary) => write!(f, "artifact transaction failed: {primary}")?,
            None => write!(f, "artifact transaction cleanup failed after commit")?,
        }
        write!(f, "; publication state: {:?}", self.publication)?;
        for failure in &self.invalidation_failures {
            write!(f, "; invalidation: {failure}")?;
        }
        for failure in &self.cleanup_failures {
            write!(f, "; cleanup: {failure}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ArtifactTransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.primary
            .as_deref()
            .map(|error| error as &(dyn std::error::Error + 'static))
            .or_else(|| {
                self.invalidation_failures
                    .first()
                    .map(|error| error as &(dyn std::error::Error + 'static))
            })
            .or_else(|| {
                self.cleanup_failures
                    .first()
                    .map(|error| error as &(dyn std::error::Error + 'static))
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenameKind {
    Artifact,
    Ownership,
}

trait TransactionHooks {
    fn before_stage_create(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn before_stage_stat(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn before_rename(&mut self, _kind: RenameKind, _completed: usize) -> io::Result<()> {
        Ok(())
    }

    fn before_invalidate(&mut self, _entry: &str) -> io::Result<()> {
        Ok(())
    }

    fn before_stage_cleanup(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn after_registry_commit(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct NoFaults;

impl TransactionHooks for NoFaults {}

struct PinnedOutput {
    fd: OwnedFd,
    display_path: PathBuf,
    device: u64,
    inode: u64,
    path_guard: Option<FilesystemPathGuardDomain>,
}

struct FilesystemPathGuardDomain {
    directory: OwnedFd,
    display_path: PathBuf,
    identity: ProcessLockIdentity,
    lock_start: u64,
    lock_length: u64,
    service_uid: u32,
}

impl FilesystemPathGuardDomain {
    fn try_clone(&self) -> Result<Self, EmitError> {
        Ok(Self {
            directory: rustix::io::fcntl_dupfd_cloexec(&self.directory, 0)
                .map_err(std::io::Error::from)?,
            display_path: self.display_path.clone(),
            identity: self.identity,
            lock_start: self.lock_start,
            lock_length: self.lock_length,
            service_uid: self.service_uid,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ProcessLockIdentity {
    device: u64,
    inode: u64,
}

impl ProcessLockIdentity {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
        }
    }
}

struct ProcessLockState {
    pid: u32,
    held: HashSet<ProcessLockIdentity>,
}

// Linux OFD locks distinguish open file descriptions, not threads. This registry avoids blocking
// one thread in the kernel behind another lock owned by this process and drives lock observations.
// Its PID reset handles the copied userspace registry after fork; the kernel OFD lock itself stays
// attached to an inherited descriptor until that descriptor is closed or CLOEXEC runs.
struct ProcessLockRegistry {
    state: Mutex<ProcessLockState>,
    released: Condvar,
}

impl ProcessLockRegistry {
    fn global() -> &'static Self {
        static REGISTRY: OnceLock<ProcessLockRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| ProcessLockRegistry {
            state: Mutex::new(ProcessLockState {
                pid: process::id(),
                held: HashSet::new(),
            }),
            released: Condvar::new(),
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, ProcessLockState> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let pid = process::id();
        if state.pid != pid {
            state.pid = pid;
            state.held.clear();
        }
        state
    }

    fn wait<'a>(
        &self,
        state: std::sync::MutexGuard<'a, ProcessLockState>,
    ) -> std::sync::MutexGuard<'a, ProcessLockState> {
        self.released
            .wait(state)
            .unwrap_or_else(|error| error.into_inner())
    }
}

struct ProcessLockReservation {
    identity: ProcessLockIdentity,
}

impl Drop for ProcessLockReservation {
    fn drop(&mut self) {
        let registry = ProcessLockRegistry::global();
        let mut state = registry.state();
        if state.held.remove(&self.identity) {
            registry.released.notify_all();
        }
    }
}

impl PinnedOutput {
    fn open(path: &Path) -> Result<Self, EmitError> {
        Self::open_with_create(path, true)
    }

    fn open_existing(path: &Path) -> Result<Self, EmitError> {
        Self::open_with_create(path, false)
    }

    fn open_with_create(path: &Path, create: bool) -> Result<Self, EmitError> {
        #[cfg(target_os = "linux")]
        let path_guard_directory = if !is_proc_self_fd_path(path) {
            let service_uid = rustix::process::geteuid().as_raw();
            let (directory, display_path, whole_domain_lock) =
                open_linux_filesystem_path_guard_directory(service_uid)?;
            let stat = fstat(&directory).map_err(std::io::Error::from)?;
            Some((
                directory,
                display_path,
                ProcessLockIdentity::from_stat(&stat),
                service_uid,
                whole_domain_lock,
            ))
        } else {
            None
        };
        let OpenedDirectoryWalk {
            directory: fd,
            path_guard_key,
        } = open_directory_walk_with_guard_key(path, create)?;
        let stat = fstat(&fd).map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
            return Err(EmitError::InvalidArtifactDestination {
                path: path.to_path_buf(),
                reason: "output path is not a directory".to_string(),
            });
        }
        #[cfg(target_os = "linux")]
        let path_guard = match (path_guard_key, path_guard_directory) {
            (
                Some(key),
                Some((directory, display_path, identity, service_uid, whole_domain_lock)),
            ) => {
                let mut offset_bytes = [0_u8; 8];
                offset_bytes.copy_from_slice(&key[..8]);
                Some(FilesystemPathGuardDomain {
                    directory,
                    display_path,
                    identity,
                    lock_start: if whole_domain_lock {
                        0
                    } else {
                        u64::from_le_bytes(offset_bytes) & (i64::MAX as u64)
                    },
                    lock_length: if whole_domain_lock { 0 } else { 1 },
                    service_uid,
                })
            }
            (None, None) => None,
            _ => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: path.to_path_buf(),
                    reason: "path-guard admission did not retain one exact coordination domain"
                        .to_owned(),
                });
            }
        };
        #[cfg(not(target_os = "linux"))]
        let path_guard: Option<FilesystemPathGuardDomain> = match path_guard_key {
            None => None,
            Some(_) => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: path.to_path_buf(),
                    reason: "artifact path guarding requires Linux".to_owned(),
                });
            }
        };
        Ok(Self {
            fd,
            display_path: path.to_path_buf(),
            device: stat.st_dev,
            inode: stat.st_ino,
            path_guard,
        })
    }

    fn verify_path_identity(&self) -> Result<(), EmitError> {
        let reopened = open_directory_walk(&self.display_path, false)?;
        let stat = fstat(&reopened).map_err(std::io::Error::from)?;
        if stat.st_dev != self.device || stat.st_ino != self.inode {
            return Err(EmitError::OutputDirectoryChanged {
                path: self.display_path.clone(),
            });
        }
        Ok(())
    }

    fn try_clone(&self) -> Result<Self, EmitError> {
        Ok(Self {
            fd: rustix::io::fcntl_dupfd_cloexec(&self.fd, 0).map_err(std::io::Error::from)?,
            display_path: self.display_path.clone(),
            device: self.device,
            inode: self.inode,
            path_guard: self
                .path_guard
                .as_ref()
                .map(FilesystemPathGuardDomain::try_clone)
                .transpose()?,
        })
    }

    fn lock(&self) -> Result<OutputLock, EmitError> {
        self.lock_with(false, None).and_then(|lock| {
            lock.ok_or_else(|| EmitError::InvalidArtifactDestination {
                path: self.display_path.join(LOCK_FILE),
                reason: "blocking lock unexpectedly reported contention".to_string(),
            })
        })
    }

    fn try_lock(&self) -> Result<Option<OutputLock>, EmitError> {
        self.lock_with(true, None)
    }

    #[cfg(feature = "test-hooks")]
    fn lock_for_build_attempt(
        &self,
        observation: &BeginBuildAttemptLockObservationV1,
    ) -> Result<OutputLock, EmitError> {
        self.lock_with(false, Some(observation)).and_then(|lock| {
            lock.ok_or_else(|| EmitError::InvalidArtifactDestination {
                path: self.display_path.join(LOCK_FILE),
                reason: "blocking lock unexpectedly reported contention".to_string(),
            })
        })
    }

    fn lock_with(
        &self,
        nonblocking: bool,
        #[cfg(feature = "test-hooks")] observation: Option<&BeginBuildAttemptLockObservationV1>,
        #[cfg(not(feature = "test-hooks"))] _observation: Option<&()>,
    ) -> Result<Option<OutputLock>, EmitError> {
        let validate_lock = |stat: &rustix::fs::Stat| {
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.join(LOCK_FILE),
                    reason: "lock entry is not a regular file".to_string(),
                });
            }
            if stat.st_nlink != 1 || stat.st_mode & 0o077 != 0 {
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.join(LOCK_FILE),
                    reason: "lock entry must be private and have exactly one link".to_string(),
                });
            }
            Ok(())
        };
        let validate_path_identity = |fd_stat: &rustix::fs::Stat| -> Result<(), EmitError> {
            let path_stat = statat(&self.fd, LOCK_FILE, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)?;
            validate_lock(&path_stat)?;
            if path_stat.st_dev != fd_stat.st_dev || path_stat.st_ino != fd_stat.st_ino {
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.join(LOCK_FILE),
                    reason: "lock entry changed while it was being acquired".to_string(),
                });
            }
            Ok(())
        };

        // The path guard is acquired first. A fresh cooperating writer that opens a replacement
        // at the same root name therefore cannot enter a disjoint root critical section while a
        // transaction still owns the displaced root descriptor.
        let path_guard = match &self.path_guard {
            Some(domain) => {
                #[cfg(feature = "test-hooks")]
                let acquired = if !nonblocking {
                    if let Some(observation) = observation {
                        match acquire_linux_filesystem_path_guard(domain, true)? {
                            Some(path_guard) => Some(path_guard),
                            None => {
                                observation
                                    .advance_to(BeginBuildAttemptLockProbeStateV1::Contended);
                                acquire_linux_filesystem_path_guard(domain, false)?
                            }
                        }
                    } else {
                        acquire_linux_filesystem_path_guard(domain, false)?
                    }
                } else {
                    acquire_linux_filesystem_path_guard(domain, true)?
                };
                #[cfg(not(feature = "test-hooks"))]
                let acquired = acquire_linux_filesystem_path_guard(domain, nonblocking)?;

                match acquired {
                    Some(path_guard) => Some(path_guard),
                    None => return Ok(None),
                }
            }
            None => None,
        };
        self.verify_path_identity()?;

        let registry = ProcessLockRegistry::global();
        let (fd, reservation) = loop {
            let mut state = registry.state();
            let path_stat = match statat(&self.fd, LOCK_FILE, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => {
                    validate_lock(&stat)?;
                    Some(stat)
                }
                Err(error) if error == rustix::io::Errno::NOENT => None,
                Err(error) => return Err(std::io::Error::from(error).into()),
            };
            let path_identity = path_stat.as_ref().map(ProcessLockIdentity::from_stat);
            if path_identity.is_some_and(|identity| state.held.contains(&identity)) {
                if nonblocking {
                    return Ok(None);
                }
                #[cfg(feature = "test-hooks")]
                if let Some(observation) = observation {
                    observation.advance_to(BeginBuildAttemptLockProbeStateV1::Contended);
                }
                drop(registry.wait(state));
                continue;
            }

            let fd = openat(
                &self.fd,
                LOCK_FILE,
                OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(std::io::Error::from)?;
            let stat = fstat(&fd).map_err(std::io::Error::from)?;
            validate_lock(&stat)?;
            validate_path_identity(&stat)?;
            let identity = ProcessLockIdentity::from_stat(&stat);
            if state.held.contains(&identity) {
                drop(fd);
                if nonblocking {
                    return Ok(None);
                }
                #[cfg(feature = "test-hooks")]
                if let Some(observation) = observation {
                    observation.advance_to(BeginBuildAttemptLockProbeStateV1::Contended);
                }
                drop(registry.wait(state));
                continue;
            }
            state.held.insert(identity);
            drop(state);
            break (fd, ProcessLockReservation { identity });
        };

        match acquire_linux_ofd_exclusive_lock(&fd, nonblocking) {
            Ok(true) => {}
            Ok(false) => {
                drop(fd);
                drop(reservation);
                return Ok(None);
            }
            Err(error) if error.kind() == io::ErrorKind::Unsupported => {
                drop(fd);
                drop(reservation);
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.join(LOCK_FILE),
                    reason: error.to_string(),
                });
            }
            Err(error) => {
                drop(fd);
                drop(reservation);
                return Err(error.into());
            }
        }
        let root_guard = match self.acquire_directory_guard(
            &self.fd,
            self.device,
            self.inode,
            nonblocking,
            "artifact output root",
        ) {
            Ok(Some(root_guard)) => root_guard,
            Ok(None) => {
                drop(fd);
                drop(reservation);
                return Ok(None);
            }
            Err(error) => {
                drop(fd);
                drop(reservation);
                return Err(error);
            }
        };
        let lock = OutputLock {
            fd: Some(fd),
            root_guard: Some(root_guard),
            path_guard,
            reservation: Some(reservation),
        };
        let locked_stat = fstat(lock.fd.as_ref().expect("lock descriptor is present"))
            .map_err(std::io::Error::from)?;
        validate_lock(&locked_stat).and_then(|()| validate_path_identity(&locked_stat))?;
        Ok(Some(lock))
    }

    fn acquire_directory_guard(
        &self,
        directory: &OwnedFd,
        device: u64,
        inode: u64,
        nonblocking: bool,
        label: &'static str,
    ) -> Result<Option<OwnedFd>, EmitError> {
        let descriptor = openat(
            directory,
            ".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        let validate_identity = |stat: &rustix::fs::Stat| -> Result<(), EmitError> {
            if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
                || stat.st_dev != device
                || stat.st_ino != inode
            {
                return Err(EmitError::OutputDirectoryChanged {
                    path: self.display_path.clone(),
                });
            }
            Ok(())
        };
        validate_identity(&fstat(&descriptor).map_err(std::io::Error::from)?)?;
        match acquire_linux_descriptor_flock(&descriptor, nonblocking) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::Unsupported => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.clone(),
                    reason: format!("{label}: {error}"),
                });
            }
            Err(error) => return Err(error.into()),
        }
        validate_identity(&fstat(&descriptor).map_err(std::io::Error::from)?)?;
        Ok(Some(descriptor))
    }
}

/// Acquires a whole-file Linux open-file-description write lock.
///
/// The returned lock is owned by this exact open file description. Closing unrelated descriptors
/// for the same inode cannot release it. A `fork` inherits the description and therefore retains
/// the lock until every inherited alias is closed; all lock descriptors are `CLOEXEC`, so a
/// successful `exec` releases the child's alias. Pre-exec child code must close inherited lock
/// descriptors or proceed directly to `exec`, and must not call this crate's lock APIs.
#[cfg(target_os = "linux")]
fn acquire_linux_ofd_exclusive_lock(fd: &OwnedFd, nonblocking: bool) -> io::Result<bool> {
    acquire_linux_ofd_exclusive_range(fd, 0, 0, nonblocking)
}

#[cfg(target_os = "linux")]
fn acquire_linux_ofd_exclusive_range(
    fd: &OwnedFd,
    start: u64,
    length: u64,
    nonblocking: bool,
) -> io::Result<bool> {
    let command = if nonblocking {
        libc::F_OFD_SETLK
    } else {
        libc::F_OFD_SETLKW
    };
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_start = libc::off_t::try_from(start).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "OFD lock start is out of range",
        )
    })?;
    lock.l_len = libc::off_t::try_from(length).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "OFD lock length is out of range",
        )
    })?;
    lock.l_pid = 0;
    loop {
        // SAFETY: `fd` is live for the call and `lock` is a fully initialized `struct flock`.
        let result = unsafe { libc::fcntl(fd.as_raw_fd(), command, &lock) };
        if result == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(libc::EACCES) | Some(libc::EAGAIN) if nonblocking => return Ok(false),
            Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::EOPNOTSUPP) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "artifact publication requires Linux F_OFD_SETLK/F_OFD_SETLKW support",
                ));
            }
            _ => return Err(error),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn acquire_linux_ofd_exclusive_lock(_fd: &OwnedFd, _nonblocking: bool) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "artifact publication requires Linux F_OFD_SETLK/F_OFD_SETLKW support",
    ))
}

/// Locks the stable admitted directory inode with Linux descriptor-owned `flock` semantics.
///
/// This guard composes with the named OFD record lock. Even if arbitrary same-UID code replaces
/// the named entry while a critical section is active, every cooperating caller for the same
/// pinned root still contends on this inode and cannot enter a split-brain critical section.
#[cfg(target_os = "linux")]
fn acquire_linux_descriptor_flock(fd: &OwnedFd, nonblocking: bool) -> io::Result<bool> {
    let operation = libc::LOCK_EX | if nonblocking { libc::LOCK_NB } else { 0 };
    loop {
        // SAFETY: `fd` is live for the call and `operation` is a valid Linux flock operation.
        if unsafe { libc::flock(fd.as_raw_fd(), operation) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(libc::EACCES) | Some(libc::EAGAIN) if nonblocking => return Ok(false),
            Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::EOPNOTSUPP) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "artifact publication requires Linux descriptor-owned directory flock support",
                ));
            }
            _ => return Err(error),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn acquire_linux_descriptor_flock(_fd: &OwnedFd, _nonblocking: bool) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "artifact publication requires Linux descriptor-owned directory flock support",
    ))
}

const FILESYSTEM_PATH_GUARD_DIRECTORY_ENV: &str = "FE2O3_ARTIFACT_PATH_GUARD_DIR";
const FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV: &str = "FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY";
const FILESYSTEM_PATH_GUARD_RUNTIME_DIRECTORY: &str = "fe2o3-artifact-path-guards-v1";
const FILESYSTEM_PATH_GUARD_HOME_DIRECTORY: &str = ".fe2o3-artifact-path-guards-v1";
const FILESYSTEM_PATH_GUARD_FILE: &str = "domain.lock";
static SAME_MOUNT_NAMESPACE_PATH_GUARD_V1: AtomicBool = AtomicBool::new(false);

/// Explicitly selects normalized-path coordination for a process whose cooperating writers all
/// observe the same mount namespace and the same normalized absolute artifact paths.
///
/// Production deployments spanning mount namespaces must instead configure
/// `FE2O3_ARTIFACT_PATH_GUARD_DIR` and `FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY` to the same
/// pre-provisioned directory inode in every namespace. Without either explicit selection,
/// ordinary path-based artifact publication fails closed.
pub fn enable_same_mount_namespace_artifact_path_guard_v1() {
    SAME_MOUNT_NAMESPACE_PATH_GUARD_V1.store(true, Ordering::Release);
}

/// Acquires one filesystem-visible OFD guard for the admitted deployment domain.
///
/// The private service-owned coordination file is intentionally outside the output tree, so
/// replacing the output or any ancestor cannot split the cooperative critical section. Writers in
/// different mount namespaces must set `FE2O3_ARTIFACT_PATH_GUARD_DIR` to one pre-provisioned `0700`
/// directory that is bind-mounted from the same inode into every namespace, and set
/// `FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY` to that directory's exact lowercase
/// `<16-hex-device>:<16-hex-inode>` identity. That configured guard file is locked as one whole
/// namespace-independent domain, so aliases such as `/a/store` and `/b/store` cannot select
/// different critical sections. Supplying only one configuration value fails closed. Processes
/// explicitly restricted to one mount namespace may call
/// [`enable_same_mount_namespace_artifact_path_guard_v1`] to use normalized-path byte ranges in a
/// service-owned runtime or home directory. Admission pins the selected directory file description
/// and identity for every later acquisition. The file is retained across acquisitions; only its
/// kernel lock is transient, so process death cannot leave a stale owner.
#[cfg(target_os = "linux")]
fn acquire_linux_filesystem_path_guard(
    domain: &FilesystemPathGuardDomain,
    nonblocking: bool,
) -> Result<Option<OwnedFd>, EmitError> {
    validate_linux_path_guard_directory(
        &domain.directory,
        &domain.display_path,
        domain.service_uid,
        true,
    )?;
    let descriptor_stat = fstat(&domain.directory).map_err(std::io::Error::from)?;
    if ProcessLockIdentity::from_stat(&descriptor_stat) != domain.identity {
        return Err(EmitError::InvalidArtifactDestination {
            path: domain.display_path.clone(),
            reason: "pinned path-guard directory identity changed after admission".to_owned(),
        });
    }
    let named_directory = open_directory_walk(&domain.display_path, false)?;
    let named_stat = fstat(&named_directory).map_err(std::io::Error::from)?;
    if ProcessLockIdentity::from_stat(&named_stat) != domain.identity {
        return Err(EmitError::InvalidArtifactDestination {
            path: domain.display_path.clone(),
            reason: "path-guard directory path no longer names its admitted inode".to_owned(),
        });
    }
    let descriptor = openat(
        &domain.directory,
        FILESYSTEM_PATH_GUARD_FILE,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(std::io::Error::from)?;
    let validate_guard = |stat: &rustix::fs::Stat| -> Result<(), EmitError> {
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_uid != domain.service_uid
            || stat.st_mode & 0o7777 != 0o600
            || stat.st_nlink != 1
        {
            return Err(EmitError::InvalidArtifactDestination {
                path: domain.display_path.join(FILESYSTEM_PATH_GUARD_FILE),
                reason: "path-guard file must be private, service-owned, and single-link"
                    .to_owned(),
            });
        }
        Ok(())
    };
    let descriptor_stat = fstat(&descriptor).map_err(std::io::Error::from)?;
    validate_guard(&descriptor_stat)?;
    if !acquire_linux_ofd_exclusive_range(
        &descriptor,
        domain.lock_start,
        domain.lock_length,
        nonblocking,
    )? {
        return Ok(None);
    }
    let named_stat = statat(
        &domain.directory,
        FILESYSTEM_PATH_GUARD_FILE,
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(std::io::Error::from)?;
    validate_guard(&named_stat)?;
    if ProcessLockIdentity::from_stat(&descriptor_stat)
        != ProcessLockIdentity::from_stat(&named_stat)
    {
        return Err(EmitError::InvalidArtifactDestination {
            path: domain.display_path.join(FILESYSTEM_PATH_GUARD_FILE),
            reason: "path-guard file changed while its byte range was being locked".to_owned(),
        });
    }
    Ok(Some(descriptor))
}

#[cfg(target_os = "linux")]
fn open_linux_filesystem_path_guard_directory(
    service_uid: u32,
) -> Result<(OwnedFd, PathBuf, bool), EmitError> {
    let configured = std::env::var_os(FILESYSTEM_PATH_GUARD_DIRECTORY_ENV);
    let configured_identity = std::env::var_os(FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV);
    if configured.is_none() != configured_identity.is_none() {
        return Err(EmitError::InvalidArtifactDestination {
            path: configured
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("<path-guard-directory>")),
            reason: format!(
                "{FILESYSTEM_PATH_GUARD_DIRECTORY_ENV} and {FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV} must be configured together"
            ),
        });
    }
    if let Some(configured) = configured {
        let path = PathBuf::from(configured);
        let directory = open_directory_walk(&path, false)?;
        validate_linux_path_guard_directory(&directory, &path, service_uid, true)?;
        let expected = configured_identity
            .and_then(|identity| parse_linux_path_guard_directory_identity(&identity))
            .ok_or_else(|| EmitError::InvalidArtifactDestination {
                path: path.clone(),
                reason: format!(
                    "{FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV} must contain the exact lowercase <16-hex-device>:<16-hex-inode> identity"
                ),
            })?;
        let stat = fstat(&directory).map_err(std::io::Error::from)?;
        if expected != (stat.st_dev, stat.st_ino) {
            return Err(EmitError::InvalidArtifactDestination {
                path,
                reason:
                    "configured path-guard directory identity does not match its provisioned inode"
                        .to_owned(),
            });
        }
        return Ok((directory, path, true));
    }

    let test_default_is_allowed =
        cfg!(test) && std::env::var_os("FE2O3_TEST_REQUIRE_EXPLICIT_PATH_GUARD").is_none();
    if !test_default_is_allowed && !SAME_MOUNT_NAMESPACE_PATH_GUARD_V1.load(Ordering::Acquire) {
        return Err(EmitError::InvalidArtifactDestination {
            path: PathBuf::from("<path-guard-directory>"),
            reason: format!(
                "ordinary artifact paths require either {FILESYSTEM_PATH_GUARD_DIRECTORY_ENV} with {FILESYSTEM_PATH_GUARD_DIRECTORY_IDENTITY_ENV}, or an explicit same-mount-namespace path-guard selection"
            ),
        });
    }

    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let runtime = PathBuf::from(runtime);
        let base = open_directory_walk(&runtime, false)?;
        validate_linux_path_guard_directory(&base, &runtime, service_uid, true)?;
        return create_linux_path_guard_directory(
            &base,
            &runtime,
            FILESYSTEM_PATH_GUARD_RUNTIME_DIRECTORY,
            service_uid,
        )
        .map(|(directory, path)| (directory, path, false));
    }

    let runtime = PathBuf::from(format!("/run/user/{service_uid}"));
    if let Ok(base) = open_directory_walk(&runtime, false) {
        validate_linux_path_guard_directory(&base, &runtime, service_uid, true)?;
        return create_linux_path_guard_directory(
            &base,
            &runtime,
            FILESYSTEM_PATH_GUARD_RUNTIME_DIRECTORY,
            service_uid,
        )
        .map(|(directory, path)| (directory, path, false));
    }

    let home = std::env::var_os("HOME").ok_or_else(|| EmitError::InvalidArtifactDestination {
        path: PathBuf::from("<path-guard-directory>"),
        reason: format!(
            "{FILESYSTEM_PATH_GUARD_DIRECTORY_ENV}, XDG_RUNTIME_DIR, and HOME are all unavailable"
        ),
    })?;
    let home = PathBuf::from(home);
    let base = open_directory_walk(&home, false)?;
    validate_linux_path_guard_directory(&base, &home, service_uid, false)?;
    create_linux_path_guard_directory(
        &base,
        &home,
        FILESYSTEM_PATH_GUARD_HOME_DIRECTORY,
        service_uid,
    )
    .map(|(directory, path)| (directory, path, false))
}

#[cfg(target_os = "linux")]
fn parse_linux_path_guard_directory_identity(identity: &std::ffi::OsStr) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = identity.as_bytes();
    if bytes.len() != 33
        || bytes[16] != b':'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 16 && !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    let device = std::str::from_utf8(&bytes[..16]).ok()?;
    let inode = std::str::from_utf8(&bytes[17..]).ok()?;
    Some((
        u64::from_str_radix(device, 16).ok()?,
        u64::from_str_radix(inode, 16).ok()?,
    ))
}

#[cfg(target_os = "linux")]
fn create_linux_path_guard_directory(
    base: &OwnedFd,
    base_path: &Path,
    name: &str,
    service_uid: u32,
) -> Result<(OwnedFd, PathBuf), EmitError> {
    match mkdirat(base, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
        Ok(()) => {}
        Err(error) if error == rustix::io::Errno::EXIST => {}
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    let path = base_path.join(name);
    let directory = openat(
        base,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    validate_linux_path_guard_directory(&directory, &path, service_uid, true)?;
    Ok((directory, path))
}

#[cfg(target_os = "linux")]
fn validate_linux_path_guard_directory(
    directory: &OwnedFd,
    path: &Path,
    service_uid: u32,
    require_private: bool,
) -> Result<(), EmitError> {
    if !path.is_absolute() {
        return Err(EmitError::InvalidArtifactDestination {
            path: path.to_path_buf(),
            reason: "path-guard directory must be absolute".to_owned(),
        });
    }
    let stat = fstat(directory).map_err(std::io::Error::from)?;
    let permissions = stat.st_mode & 0o7777;
    let permitted = if require_private {
        permissions == 0o700
    } else {
        permissions & 0o022 == 0
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != service_uid
        || stat.st_nlink == 0
        || !permitted
    {
        return Err(EmitError::InvalidArtifactDestination {
            path: path.to_path_buf(),
            reason: if require_private {
                "path-guard directory must be linked, service-owned, and mode 0700".to_owned()
            } else {
                "path-guard parent must be linked, service-owned, and not group/world writable"
                    .to_owned()
            },
        });
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn acquire_linux_filesystem_path_guard(
    _domain: &FilesystemPathGuardDomain,
    _nonblocking: bool,
) -> Result<Option<OwnedFd>, EmitError> {
    Err(EmitError::InvalidArtifactDestination {
        path: PathBuf::from("<filesystem-path-guard>"),
        reason: "artifact publication requires Linux OFD byte-range locks".to_owned(),
    })
}

struct OpenedDirectoryWalk {
    directory: OwnedFd,
    path_guard_key: Option<[u8; 32]>,
}

fn open_directory_walk(path: &Path, create: bool) -> Result<OwnedFd, EmitError> {
    Ok(open_directory_walk_with_guard_key(path, create)?.directory)
}

fn open_directory_walk_with_guard_key(
    path: &Path,
    create: bool,
) -> Result<OpenedDirectoryWalk, EmitError> {
    #[cfg(target_os = "linux")]
    if let Some(directory) = duplicate_proc_self_fd_directory(path) {
        let directory = directory?;
        return Ok(OpenedDirectoryWalk {
            directory,
            path_guard_key: None,
        });
    }

    if !path.is_absolute() {
        return Err(EmitError::InvalidArtifactDestination {
            path: path.to_path_buf(),
            reason: "artifact output paths must be absolute so path locking and descriptor traversal share one identity"
                .to_owned(),
        });
    }
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => names.push(name),
            Component::ParentDir => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: path.to_path_buf(),
                    reason: "parent-directory components are not allowed".to_string(),
                });
            }
            Component::Prefix(_) => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: path.to_path_buf(),
                    reason: "platform path prefixes are not supported".to_string(),
                });
            }
        }
    }

    let mut current = open(
        Path::new("/"),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;

    for name in names {
        let open_component = || {
            openat(
                &current,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
        };
        current = match open_component() {
            Ok(fd) => fd,
            Err(error) if create && error == rustix::io::Errno::NOENT => {
                match mkdirat(
                    &current,
                    name,
                    Mode::RUSR
                        | Mode::WUSR
                        | Mode::XUSR
                        | Mode::RGRP
                        | Mode::XGRP
                        | Mode::ROTH
                        | Mode::XOTH,
                ) {
                    Ok(()) => {}
                    Err(error) if error == rustix::io::Errno::EXIST => {}
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
                open_component().map_err(std::io::Error::from)?
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
    }
    Ok(OpenedDirectoryWalk {
        directory: current,
        path_guard_key: Some(normalized_absolute_path_guard_key(path)?),
    })
}

fn normalized_absolute_path_guard_key(path: &Path) -> Result<[u8; 32], EmitError> {
    use std::os::unix::ffi::OsStrExt;

    if !path.is_absolute() {
        return Err(EmitError::InvalidArtifactDestination {
            path: path.to_path_buf(),
            reason: "path-guard key requires an absolute Unix path".to_owned(),
        });
    }
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/ARTIFACT-ABSOLUTE-PATH-GUARD/V1\0");
    digest.update(rustix::process::geteuid().as_raw().to_le_bytes());
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                let bytes = name.as_bytes();
                digest.update((bytes.len() as u64).to_le_bytes());
                digest.update(bytes);
            }
            Component::ParentDir | Component::Prefix(_) => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: path.to_path_buf(),
                    reason: "path-guard key requires a normalized Unix path".to_owned(),
                });
            }
        }
    }
    Ok(digest.finalize().into())
}

#[cfg(target_os = "linux")]
fn is_proc_self_fd_path(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().starts_with(b"/proc/self/fd/")
}

#[cfg(target_os = "linux")]
fn duplicate_proc_self_fd_directory(path: &Path) -> Option<Result<OwnedFd, EmitError>> {
    use std::os::unix::ffi::OsStrExt;

    const PREFIX: &[u8] = b"/proc/self/fd/";
    let bytes = path.as_os_str().as_bytes();
    let descriptor = bytes.strip_prefix(PREFIX)?;
    let canonical = descriptor == b"0"
        || descriptor
            .first()
            .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
            && descriptor.iter().all(u8::is_ascii_digit);
    if !canonical {
        return Some(Err(EmitError::InvalidArtifactDestination {
            path: path.to_path_buf(),
            reason: "procfs descriptor path is not canonical".to_string(),
        }));
    }

    let raw_fd = descriptor.iter().try_fold(0_i32, |value, digit| {
        value.checked_mul(10)?.checked_add(i32::from(*digit - b'0'))
    });
    let Some(raw_fd) = raw_fd else {
        return Some(Err(EmitError::InvalidArtifactDestination {
            path: path.to_path_buf(),
            reason: "procfs descriptor number is out of range".to_string(),
        }));
    };

    // Raw fcntl reports EBADF for stale descriptor numbers without manufacturing a BorrowedFd
    // whose validity contract would already have been violated.
    let duplicated = unsafe { libc::fcntl(raw_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicated < 0 {
        return Some(Err(std::io::Error::last_os_error().into()));
    }
    let directory = unsafe { OwnedFd::from_raw_fd(duplicated) };
    let stat = match fstat(&directory) {
        Ok(stat) => stat,
        Err(error) => return Some(Err(std::io::Error::from(error).into())),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return Some(Err(EmitError::InvalidArtifactDestination {
            path: path.to_path_buf(),
            reason: "procfs descriptor does not reference a directory".to_string(),
        }));
    }
    Some(Ok(directory))
}

struct OutputLock {
    fd: Option<OwnedFd>,
    root_guard: Option<OwnedFd>,
    path_guard: Option<OwnedFd>,
    reservation: Option<ProcessLockReservation>,
}

impl Drop for OutputLock {
    fn drop(&mut self) {
        ArtifactProcessSpawnCoordinatorV1::global().release_lock_descriptors(|| {
            drop(self.fd.take());
            drop(self.root_guard.take());
            drop(self.path_guard.take());
            drop(self.reservation.take());
        });
    }
}

struct StagingDirectory {
    output_fd: OwnedFd,
    fd: OwnedFd,
    subprocess_fd: OwnedFd,
    name: String,
    device: u64,
    inode: u64,
    active: bool,
}

struct StagingCreateError {
    primary: EmitError,
    cleanup_failures: Vec<FilesystemFailure>,
}

impl StagingCreateError {
    fn new(primary: impl Into<EmitError>) -> Self {
        Self {
            primary: primary.into(),
            cleanup_failures: Vec::new(),
        }
    }
}

impl StagingDirectory {
    fn create(
        output: &PinnedOutput,
        hooks: &mut impl TransactionHooks,
    ) -> Result<Self, StagingCreateError> {
        hooks
            .before_stage_create()
            .map_err(|error| StagingCreateError::new(EmitError::from(error)))?;
        let start = NEXT_STAGING_ID.fetch_add(MAX_STAGING_ATTEMPTS, Ordering::Relaxed);
        for offset in 0..MAX_STAGING_ATTEMPTS {
            let name = format!(
                "{STAGING_PREFIX}{}-{}",
                process::id(),
                start.wrapping_add(offset)
            );
            match mkdirat(
                &output.fd,
                name.as_str(),
                Mode::RUSR | Mode::WUSR | Mode::XUSR,
            ) {
                Ok(()) => {
                    let fd = match openat(
                        &output.fd,
                        name.as_str(),
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    ) {
                        Ok(fd) => fd,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(
                                std::io::Error::from(error),
                            ));
                            if let Err(cleanup_error) =
                                unlinkat(&output.fd, name.as_str(), AtFlags::REMOVEDIR)
                            {
                                failure.cleanup_failures.push(FilesystemFailure {
                                    operation: "remove unopened staging directory",
                                    entry: name,
                                    error: cleanup_error.into(),
                                });
                            }
                            return Err(failure);
                        }
                    };
                    let stat = match hooks
                        .before_stage_stat()
                        .and_then(|()| fstat(&fd).map_err(std::io::Error::from))
                    {
                        Ok(stat) => stat,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(error));
                            match fstat(&fd) {
                                Ok(stat) => {
                                    failure.cleanup_failures.extend(cleanup_created_staging(
                                        output,
                                        &fd,
                                        &name,
                                        stat.st_dev,
                                        stat.st_ino,
                                        hooks,
                                    ))
                                }
                                Err(cleanup_error) => {
                                    failure.cleanup_failures.push(FilesystemFailure {
                                        operation: "identify staging directory for cleanup",
                                        entry: name,
                                        error: cleanup_error.into(),
                                    });
                                }
                            }
                            return Err(failure);
                        }
                    };
                    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
                        || stat.st_mode & 0o777 != 0o700
                    {
                        let mut failure =
                            StagingCreateError::new(EmitError::InvalidArtifactDestination {
                                path: output.display_path.join(&name),
                                reason: "staging directory is not a private 0700 directory"
                                    .to_string(),
                            });
                        failure.cleanup_failures.extend(cleanup_created_staging(
                            output,
                            &fd,
                            &name,
                            stat.st_dev,
                            stat.st_ino,
                            hooks,
                        ));
                        return Err(failure);
                    }
                    let output_fd = match rustix::io::fcntl_dupfd_cloexec(&output.fd, 0) {
                        Ok(fd) => fd,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(
                                std::io::Error::from(error),
                            ));
                            failure.cleanup_failures.extend(cleanup_created_staging(
                                output,
                                &fd,
                                &name,
                                stat.st_dev,
                                stat.st_ino,
                                hooks,
                            ));
                            return Err(failure);
                        }
                    };
                    let subprocess_fd = match rustix::io::dup(&fd) {
                        Ok(fd) => fd,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(
                                std::io::Error::from(error),
                            ));
                            failure.cleanup_failures.extend(cleanup_created_staging(
                                output,
                                &fd,
                                &name,
                                stat.st_dev,
                                stat.st_ino,
                                hooks,
                            ));
                            return Err(failure);
                        }
                    };
                    let proc_path = format!("/proc/self/fd/{}", subprocess_fd.as_raw_fd());
                    if !Path::new(&proc_path).is_dir() {
                        let mut failure =
                            StagingCreateError::new(EmitError::SubprocessPathBoundary {
                                reason: format!("pinned directory path {proc_path} is unavailable"),
                            });
                        failure.cleanup_failures.extend(cleanup_created_staging(
                            output,
                            &fd,
                            &name,
                            stat.st_dev,
                            stat.st_ino,
                            hooks,
                        ));
                        return Err(failure);
                    }
                    return Ok(Self {
                        output_fd,
                        fd,
                        subprocess_fd,
                        name,
                        device: stat.st_dev,
                        inode: stat.st_ino,
                        active: true,
                    });
                }
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(error) => {
                    return Err(StagingCreateError::new(EmitError::from(
                        std::io::Error::from(error),
                    )));
                }
            }
        }
        Err(StagingCreateError::new(EmitError::StagingExhausted {
            output_dir: output.display_path.clone(),
        }))
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), EmitError> {
        let fd = openat(
            &self.fd,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(std::io::Error::from)?;
        let mut file = fs::File::from(fd);
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }

    fn replace(&self, name: &str, bytes: &[u8]) -> Result<(), EmitError> {
        match unlinkat(&self.fd, name, AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        self.write(name, bytes)
    }

    fn subprocess_path(&self, name: &str) -> PathBuf {
        PathBuf::from(format!(
            "/proc/self/fd/{}/{}",
            self.subprocess_fd.as_raw_fd(),
            name
        ))
    }

    fn cleanup(&mut self, hooks: &mut impl TransactionHooks) -> Vec<FilesystemFailure> {
        if !self.active {
            return Vec::new();
        }
        if let Err(error) = hooks.before_stage_cleanup() {
            return vec![FilesystemFailure {
                operation: "remove staging directory",
                entry: self.name.clone(),
                error,
            }];
        }
        let failures = cleanup_staging(
            &self.output_fd,
            &self.fd,
            &self.name,
            self.device,
            self.inode,
        );
        if failures.is_empty() {
            self.active = false;
        }
        failures
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if self.active {
            let failures = cleanup_staging(
                &self.output_fd,
                &self.fd,
                &self.name,
                self.device,
                self.inode,
            );
            if failures.is_empty() {
                self.active = false;
            }
        }
    }
}

fn cleanup_created_staging(
    output: &PinnedOutput,
    staging_fd: &OwnedFd,
    staging_name: &str,
    staging_device: u64,
    staging_inode: u64,
    hooks: &mut impl TransactionHooks,
) -> Vec<FilesystemFailure> {
    if let Err(error) = hooks.before_stage_cleanup() {
        return vec![FilesystemFailure {
            operation: "remove staging directory after setup failure",
            entry: staging_name.to_string(),
            error,
        }];
    }
    cleanup_staging(
        &output.fd,
        staging_fd,
        staging_name,
        staging_device,
        staging_inode,
    )
}

fn cleanup_staging(
    output_fd: &OwnedFd,
    staging_fd: &OwnedFd,
    staging_name: &str,
    staging_device: u64,
    staging_inode: u64,
) -> Vec<FilesystemFailure> {
    let mut failures = Vec::new();
    let mut directory = match Dir::read_from(staging_fd) {
        Ok(directory) => directory,
        Err(error) => {
            failures.push(FilesystemFailure {
                operation: "read staging directory",
                entry: staging_name.to_string(),
                error: error.into(),
            });
            return failures;
        }
    };
    for entry in &mut directory {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(FilesystemFailure {
                    operation: "read staging entry",
                    entry: staging_name.to_string(),
                    error: error.into(),
                });
                continue;
            }
        };
        if entry.file_name().to_bytes() == b"." || entry.file_name().to_bytes() == b".." {
            continue;
        }
        let flags = if entry.file_type() == FileType::Directory {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        if let Err(error) = unlinkat(staging_fd, entry.file_name(), flags) {
            failures.push(FilesystemFailure {
                operation: "remove staging entry",
                entry: entry.file_name().to_string_lossy().into_owned(),
                error: error.into(),
            });
        }
    }
    if failures.is_empty() {
        match statat(output_fd, staging_name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if stat.st_dev == staging_device && stat.st_ino == staging_inode => {
                if let Err(error) = unlinkat(output_fd, staging_name, AtFlags::REMOVEDIR) {
                    failures.push(FilesystemFailure {
                        operation: "remove staging directory",
                        entry: staging_name.to_string(),
                        error: error.into(),
                    });
                }
            }
            Ok(_) => failures.push(FilesystemFailure {
                operation: "verify staging directory identity",
                entry: staging_name.to_string(),
                error: io::Error::other("staging directory name was substituted"),
            }),
            Err(error) => failures.push(FilesystemFailure {
                operation: "verify staging directory identity",
                entry: staging_name.to_string(),
                error: error.into(),
            }),
        }
    }
    failures
}

fn is_staging_name(name: &[u8]) -> bool {
    let Some(rest) = name.strip_prefix(STAGING_PREFIX.as_bytes()) else {
        return false;
    };
    let Some(separator) = rest.iter().position(|byte| *byte == b'-') else {
        return false;
    };
    let (pid, sequence_with_separator) = rest.split_at(separator);
    let sequence = &sequence_with_separator[1..];
    !pid.is_empty()
        && !sequence.is_empty()
        && pid.iter().all(u8::is_ascii_digit)
        && sequence.iter().all(u8::is_ascii_digit)
}

fn output_scan_fd(output: &PinnedOutput) -> Result<OwnedFd, EmitError> {
    openat(
        &output.fd,
        Path::new("."),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)
    .map_err(EmitError::from)
}

fn remove_abandoned_recovery(
    output: &PinnedOutput,
    entry: &str,
    description: &str,
) -> Result<bool, EmitError> {
    match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat)
            if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                && stat.st_nlink == 1
                && stat.st_mode & 0o077 == 0 =>
        {
            unlinkat(&output.fd, entry, AtFlags::empty()).map_err(std::io::Error::from)?;
            Ok(true)
        }
        Ok(_) => Err(EmitError::InvalidArtifactDestination {
            path: output.display_path.join(entry),
            reason: format!(
                "abandoned {description} recovery entry is not a private single-link file"
            ),
        }),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(false),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

fn cleanup_abandoned_staging(output: &PinnedOutput) -> Result<(), EmitError> {
    let ownership_recovery_removed =
        remove_abandoned_recovery(output, RECOVERY_OWNERSHIP_FILE, "ownership")?;
    let scan_fd = output_scan_fd(output)?;
    let mut directory = Dir::read_from(&scan_fd).map_err(std::io::Error::from)?;
    let mut names = Vec::new();
    let mut entries = 0usize;
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        entries = entries
            .checked_add(1)
            .ok_or_else(|| ownership_error("managed artifact directory entry count overflow"))?;
        if entries > MAX_OUTPUT_ENTRIES {
            return Err(ownership_error(
                "managed artifact directory exceeds its entry bound",
            ));
        }
        if is_staging_name(name) {
            names.push(String::from_utf8(name.to_vec()).expect("staging names are ASCII"));
        }
    }

    let mut removed_any = ownership_recovery_removed;
    for name in names {
        let stat = match statat(&output.fd, &name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(error) if error == rustix::io::Errno::NOENT => continue,
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            if stat.st_mode & 0o777 != 0o700 {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(&name),
                    reason: "abandoned staging directory is not private 0700".to_string(),
                });
            }
            let fd = openat(
                &output.fd,
                &name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
            let opened = fstat(&fd).map_err(std::io::Error::from)?;
            if opened.st_dev != stat.st_dev || opened.st_ino != stat.st_ino {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(&name),
                    reason: "abandoned staging directory changed while opening".to_string(),
                });
            }
            let failures = cleanup_staging(&output.fd, &fd, &name, opened.st_dev, opened.st_ino);
            if !failures.is_empty() {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(&name),
                    reason: failures
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; "),
                });
            }
        } else {
            unlinkat(&output.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        }
        removed_any = true;
    }
    if removed_any {
        fsync(&output.fd).map_err(std::io::Error::from)?;
    }
    Ok(())
}

fn canonical_artifact_kernel(name: &[u8]) -> Option<String> {
    let name = std::str::from_utf8(name).ok()?;
    let kernel = [".ll", ".o", ".hsaco"]
        .into_iter()
        .find_map(|extension| name.strip_suffix(extension))?;
    validate_artifact_name(kernel).ok()?;
    Some(kernel.to_string())
}

fn inventory_unowned_artifacts(
    output: &PinnedOutput,
    registry: &OwnershipRegistry,
) -> Result<BTreeSet<String>, EmitError> {
    let scan_fd = output_scan_fd(output)?;
    let mut directory = Dir::read_from(&scan_fd).map_err(std::io::Error::from)?;
    let mut kernels = BTreeSet::new();
    let mut entries = 0usize;
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        entries = entries
            .checked_add(1)
            .ok_or_else(|| ownership_error("managed artifact directory entry count overflow"))?;
        if entries > MAX_OUTPUT_ENTRIES {
            return Err(ownership_error(
                "managed artifact directory exceeds its entry bound",
            ));
        }
        let Some(kernel) = canonical_artifact_kernel(name) else {
            continue;
        };
        if registry.owner_of(&kernel).is_none() {
            kernels.insert(kernel);
            if kernels.len() > MAX_TOTAL_OWNED_KERNELS {
                return Err(ownership_error(
                    "managed artifact directory has too many unowned kernels",
                ));
            }
        }
    }
    Ok(kernels)
}

#[derive(Clone, Debug)]
struct ArtifactNames {
    kernel: String,
    llvm_ir: String,
    object: String,
    hsaco: String,
}

impl ArtifactNames {
    fn new(kernel: &str) -> Result<Self, EmitError> {
        validate_artifact_name(kernel)?;
        Ok(Self {
            kernel: kernel.to_string(),
            llvm_ir: format!("{kernel}.ll"),
            object: format!("{kernel}.o"),
            hsaco: format!("{kernel}.hsaco"),
        })
    }

    fn files(&self) -> [&str; 3] {
        [&self.llvm_ir, &self.object, &self.hsaco]
    }
}

#[derive(Debug)]
struct PreparedArtifact {
    names: ArtifactNames,
    llvm_ir: String,
}

struct PinnedArtifactSnapshot {
    llvm_ir: PinnedFinalizedFile,
    hsaco: PinnedFinalizedFile,
}

impl PinnedArtifactSnapshot {
    fn open(
        staging: &StagingDirectory,
        output: &PinnedOutput,
        names: &ArtifactNames,
    ) -> Result<Self, EmitError> {
        Ok(Self {
            llvm_ir: PinnedFinalizedFile::open(
                staging,
                output,
                &names.llvm_ir,
                MAX_FINALIZED_LLVM_IR_BYTES,
            )?,
            hsaco: PinnedFinalizedFile::open(
                staging,
                output,
                &names.hsaco,
                MAX_FINALIZED_HSACO_BYTES,
            )?,
        })
    }

    fn materialize(
        self,
        output: &PinnedOutput,
        names: &ArtifactNames,
    ) -> Result<(FinalizedArtifactSnapshot, FinalizedArtifactSnapshot), EmitError> {
        let llvm_ir = self.llvm_ir.materialize(output, &names.llvm_ir)?;
        let hsaco = self.hsaco.materialize(output, &names.hsaco)?;
        Ok((llvm_ir, hsaco))
    }
}

#[derive(Clone, Copy)]
struct FinalizedFileIdentity {
    device: u64,
    inode: u64,
    length: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl FinalizedFileIdentity {
    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            length: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }

    fn matches(self, stat: &rustix::fs::Stat) -> bool {
        self.matches_pinned(stat)
            && self.changed_seconds == stat.st_ctime
            && self.changed_nanoseconds == stat.st_ctime_nsec
    }

    fn matches_pinned(self, stat: &rustix::fs::Stat) -> bool {
        FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
            && stat.st_nlink == 1
            && self.device == stat.st_dev
            && self.inode == stat.st_ino
            && self.length == stat.st_size
            && self.modified_seconds == stat.st_mtime
            && self.modified_nanoseconds == stat.st_mtime_nsec
    }
}

struct PinnedFinalizedFile {
    file: fs::File,
    identity: FinalizedFileIdentity,
    path: PathBuf,
    expected: usize,
    maximum: usize,
}

impl PinnedFinalizedFile {
    fn open(
        staging: &StagingDirectory,
        output: &PinnedOutput,
        entry: &str,
        maximum: usize,
    ) -> Result<Self, EmitError> {
        let path = output.display_path.join(entry);
        let fd = openat(
            &staging.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| EmitError::InvalidFinalizedArtifact {
            path: path.clone(),
            reason: std::io::Error::from(error).to_string(),
        })?;
        let stat = fstat(&fd).map_err(std::io::Error::from)?;
        let identity = FinalizedFileIdentity::from_stat(&stat);
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1 {
            return Err(EmitError::InvalidFinalizedArtifact {
                path,
                reason: "expected a single-link regular file".to_string(),
            });
        }
        let expected = usize::try_from(stat.st_size).unwrap_or(usize::MAX);
        if expected == 0 || expected > maximum {
            return Err(EmitError::InvalidFinalizedArtifact {
                path,
                reason: format!("size {expected} is outside 1..={maximum} bytes"),
            });
        }
        Ok(Self {
            file: fs::File::from(fd),
            identity,
            path,
            expected,
            maximum,
        })
    }

    fn materialize(
        mut self,
        output: &PinnedOutput,
        entry: &str,
    ) -> Result<FinalizedArtifactSnapshot, EmitError> {
        let opened = fstat(&self.file).map_err(std::io::Error::from)?;
        let published =
            statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        if !self.identity.matches_pinned(&opened) || !self.identity.matches_pinned(&published) {
            return Err(EmitError::InvalidFinalizedArtifact {
                path: self.path,
                reason: "pinned file no longer matches the published generation".to_string(),
            });
        }
        let published_identity = FinalizedFileIdentity::from_stat(&opened);
        if !published_identity.matches(&published) {
            return Err(EmitError::InvalidFinalizedArtifact {
                path: self.path,
                reason: "pinned file metadata differs from the published generation".to_string(),
            });
        }

        let mut bytes = Vec::with_capacity(self.expected);
        Read::by_ref(&mut self.file)
            .take((self.maximum as u64) + 1)
            .read_to_end(&mut bytes)?;
        let completed = fstat(&self.file).map_err(std::io::Error::from)?;
        let still_published =
            statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        if bytes.len() != self.expected
            || !published_identity.matches(&completed)
            || !published_identity.matches(&still_published)
        {
            return Err(EmitError::InvalidFinalizedArtifact {
                path: self.path,
                reason: "pinned file changed while its bytes were captured".to_string(),
            });
        }
        Ok(FinalizedArtifactSnapshot::from_bytes(self.path, bytes))
    }
}

#[derive(Clone, Copy)]
struct BackendRequest<'a> {
    producer: &'a ProducerIdentity,
    attempt: Option<BuildAttempt>,
}

/// Prepares, compiles, and publishes one producer's complete kernel set.
///
/// The validated output-directory lock is held while `prepare` and `compile` run and until
/// publication or rollback completes. Callbacks must not reenter this artifact store. Each
/// successful compiler callback must create the requested `.o` and `.hsaco` beside its staged
/// `.ll`. Passing an empty kernel set reconciles the producer to no outputs and also removes
/// ownerless legacy artifacts in the managed namespace. This direct form is only for producers
/// that have never entered the cargo-managed build-attempt protocol in this output directory;
/// managed producers must continue using the attempt-authorized form.
pub fn emit_artifact_transaction<T>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    kernels: &[T],
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    emit_artifact_transaction_with_hooks(
        output_dir,
        BackendRequest {
            producer,
            attempt: None,
        },
        kernels,
        kernel_name,
        prepare,
        compile,
        &mut NoFaults,
    )
}

/// Attempt-authorized form of [`emit_artifact_transaction`].
///
/// The token must name the current claimable generation for `producer`. The backend durably
/// consumes that authorization before preflight or artifact mutation, so a crash cannot make the
/// token reusable.
pub fn emit_artifact_transaction_for_attempt<T>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    kernels: &[T],
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    emit_artifact_transaction_with_hooks(
        output_dir,
        BackendRequest {
            producer,
            attempt: Some(attempt),
        },
        kernels,
        kernel_name,
        prepare,
        compile,
        &mut NoFaults,
    )
}

/// Runs fallible collection before preparing, compiling, and publishing a complete kernel set.
///
/// The validated output-directory lock is acquired before `preflight` and retained through all
/// callbacks, publication, and rollback. This guarantees that a preflight failure invalidates the
/// producer's previous generation under the same lock. Callbacks must not reenter this artifact
/// store. See [`emit_artifact_transaction`] for staged-output and empty-set semantics.
pub fn emit_artifact_transaction_after_preflight<T, P>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    preflight: impl FnOnce() -> Result<P, EmitError>,
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError>
where
    P: AsRef<[T]>,
{
    emit_artifact_transaction_after_preflight_with_hooks(
        output_dir,
        BackendRequest {
            producer,
            attempt: None,
        },
        preflight,
        kernel_name,
        prepare,
        compile,
        &mut NoFaults,
    )
}

/// Attempt-authorized form of [`emit_artifact_transaction_after_preflight`].
pub fn emit_artifact_transaction_after_preflight_for_attempt<T, P>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    preflight: impl FnOnce() -> Result<P, EmitError>,
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError>
where
    P: AsRef<[T]>,
{
    emit_artifact_transaction_after_preflight_with_hooks(
        output_dir,
        BackendRequest {
            producer,
            attempt: Some(attempt),
        },
        preflight,
        kernel_name,
        prepare,
        compile,
        &mut NoFaults,
    )
}

fn emit_artifact_transaction_with_hooks<T>(
    output_dir: &Path,
    request: BackendRequest<'_>,
    kernels: &[T],
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
    hooks: &mut impl TransactionHooks,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    emit_artifact_transaction_after_preflight_with_hooks(
        output_dir,
        request,
        || Ok(kernels),
        kernel_name,
        prepare,
        compile,
        hooks,
    )
}

fn emit_artifact_transaction_after_preflight_with_hooks<T, P>(
    output_dir: &Path,
    request: BackendRequest<'_>,
    preflight: impl FnOnce() -> Result<P, EmitError>,
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
    hooks: &mut impl TransactionHooks,
) -> Result<Vec<DeviceArtifact>, EmitError>
where
    P: AsRef<[T]>,
{
    let BackendRequest {
        producer,
        attempt: supplied_attempt,
    } = request;
    let output = PinnedOutput::open(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let (attempt, externally_managed) =
        prepare_backend_attempt_locked(&output, producer, supplied_attempt)?;
    if let Err(primary) = cleanup_abandoned_staging(&output) {
        return finish_backend_attempt_locked(
            &output,
            producer,
            attempt,
            externally_managed,
            Err(primary),
            hooks,
        );
    }
    let result = emit_artifact_transaction_locked(
        &output,
        producer,
        preflight,
        kernel_name,
        prepare,
        compile,
        hooks,
    );
    finish_backend_attempt_locked(
        &output,
        producer,
        attempt,
        externally_managed,
        result,
        hooks,
    )
}

fn emit_artifact_transaction_locked<T, P>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    preflight: impl FnOnce() -> Result<P, EmitError>,
    kernel_name: impl Fn(&T) -> &str,
    mut prepare: impl FnMut(&T) -> Result<String, EmitError>,
    mut compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
    hooks: &mut impl TransactionHooks,
) -> Result<Vec<DeviceArtifact>, EmitError>
where
    P: AsRef<[T]>,
{
    let mut original_registry = read_registry(output)?;
    if prune_absent_ownership(output, &mut original_registry)? {
        commit_registry_direct(output, &original_registry)?;
    }
    let orphaned = inventory_unowned_artifacts(output, &original_registry)?;
    let old_owned = original_registry.owned_by(producer);

    let preflight = match preflight() {
        Ok(preflight) => preflight,
        Err(error) => {
            let invalidation_set = old_owned.union(&orphaned).cloned().collect::<BTreeSet<_>>();
            return Err(abort_without_staging(
                output,
                &original_registry,
                producer,
                &invalidation_set,
                &old_owned,
                error,
                hooks,
            ));
        }
    };
    let kernels = preflight.as_ref();
    if kernels.len() > MAX_KERNELS_PER_PRODUCER {
        let invalidation_set = old_owned.union(&orphaned).cloned().collect::<BTreeSet<_>>();
        return Err(abort_without_staging(
            output,
            &original_registry,
            producer,
            &invalidation_set,
            &old_owned,
            ownership_error("too many kernels in one compiler transaction"),
            hooks,
        ));
    }

    let mut names = Vec::with_capacity(kernels.len());
    let mut used_names = HashSet::with_capacity(kernels.len());
    let mut primary = None;
    for kernel in kernels {
        let name = kernel_name(kernel);
        match ArtifactNames::new(name) {
            Ok(artifact_names) => {
                if !used_names.insert(name.to_ascii_lowercase()) && primary.is_none() {
                    primary = Some(EmitError::DuplicateArtifactName {
                        kernel: name.to_string(),
                    });
                }
                names.push(artifact_names);
            }
            Err(error) if primary.is_none() => primary = Some(error),
            Err(_) => {}
        }
    }

    let mut protected_names = BTreeSet::new();
    for artifact in &names {
        match original_registry.owner_of(&artifact.kernel) {
            Some(owner) if owner != producer.stable_source => {
                protected_names.insert(artifact.kernel.clone());
                if primary.is_none() {
                    primary = Some(EmitError::ArtifactOwnedByOtherProducer {
                        kernel: artifact.kernel.clone(),
                    });
                }
            }
            Some(_) => {
                if let Err(error) = validate_owned_destinations(output, artifact)
                    && primary.is_none()
                {
                    primary = Some(error);
                }
            }
            // The output directory is a generated-artifact namespace. Files from the
            // pre-registry emitter are adopted on success and invalidated on failure.
            None => {}
        }
    }

    let new_owned = names
        .iter()
        .map(|artifact| artifact.kernel.clone())
        .collect::<BTreeSet<_>>();
    let recovery_candidates = old_owned
        .union(&new_owned)
        .filter(|kernel| !protected_names.contains(*kernel))
        .cloned()
        .collect::<BTreeSet<_>>();
    let invalidation_set = recovery_candidates
        .iter()
        .chain(orphaned.iter())
        .cloned()
        .collect::<BTreeSet<_>>();

    if old_owned.is_empty() && names.is_empty() && orphaned.is_empty() && primary.is_none() {
        return Ok(Vec::new());
    }

    let rollback = RollbackContext {
        output,
        original_registry: &original_registry,
        producer,
        invalidation_set: &invalidation_set,
        recovery_candidates: &recovery_candidates,
    };

    let mut staging = match StagingDirectory::create(output, hooks) {
        Ok(staging) => staging,
        Err(staging_error) => {
            let StagingCreateError {
                primary: staging_primary,
                mut cleanup_failures,
            } = staging_error;
            let abort_primary = match primary {
                Some(primary) => {
                    cleanup_failures.push(FilesystemFailure {
                        operation: "create staging directory",
                        entry: output.display_path.display().to_string(),
                        error: io::Error::other(staging_primary.to_string()),
                    });
                    primary
                }
                None => staging_primary,
            };
            let mut error = abort_without_staging(
                output,
                &original_registry,
                producer,
                &invalidation_set,
                &recovery_candidates,
                abort_primary,
                hooks,
            );
            if let EmitError::Transaction(transaction) = &mut error {
                transaction.cleanup_failures.splice(0..0, cleanup_failures);
                transaction.publication = PublicationState::NotStarted {
                    total_final_renames: names.len() * 3,
                };
            }
            return Err(error);
        }
    };

    if let Some(error) = primary {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: names.len() * 3,
            },
            error,
            hooks,
        ));
    }

    let mut prepared = Vec::with_capacity(kernels.len());
    for (kernel, artifact_names) in kernels.iter().zip(names) {
        match prepare(kernel) {
            Ok(llvm_ir) => prepared.push(PreparedArtifact {
                names: artifact_names,
                llvm_ir,
            }),
            Err(error) => {
                return Err(rollback.abort(
                    &mut staging,
                    PublicationState::NotStarted {
                        total_final_renames: kernels.len() * 3,
                    },
                    error,
                    hooks,
                ));
            }
        }
    }

    let mut pinned_snapshots = Vec::with_capacity(prepared.len());
    for artifact in &prepared {
        let result = (|| {
            staging.write(&artifact.names.llvm_ir, artifact.llvm_ir.as_bytes())?;
            let llvm_ir_path = staging.subprocess_path(&artifact.names.llvm_ir);
            let hsaco_path = staging.subprocess_path(&artifact.names.hsaco);
            compile(&llvm_ir_path, &hsaco_path)?;
            validate_staged_artifacts(&staging, &artifact.names, output)?;
            PinnedArtifactSnapshot::open(&staging, output, &artifact.names)
        })();
        match result {
            Ok(snapshot) => pinned_snapshots.push(snapshot),
            Err(error) => {
                return Err(rollback.abort(
                    &mut staging,
                    PublicationState::NotStarted {
                        total_final_renames: prepared.len() * 3,
                    },
                    error,
                    hooks,
                ));
            }
        }
    }

    let mut next_registry = original_registry.clone();
    next_registry.set_owned(producer, new_owned.clone());
    if let Err(error) = stage_registry(&staging, &next_registry) {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: prepared.len() * 3,
            },
            error,
            hooks,
        ));
    }
    if let Err(error) = fsync(&staging.fd).map_err(std::io::Error::from) {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: prepared.len() * 3,
            },
            error.into(),
            hooks,
        ));
    }

    if let Err(error) = output.verify_path_identity() {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: prepared.len() * 3,
            },
            error,
            hooks,
        ));
    }

    let total_final_renames = prepared.len() * 3;
    let mut completed_final_renames = 0usize;
    for artifact in &prepared {
        for entry in artifact.names.files() {
            if let Err(error) = hooks
                .before_rename(RenameKind::Artifact, completed_final_renames)
                .and_then(|()| {
                    renameat(&staging.fd, entry, &output.fd, entry).map_err(std::io::Error::from)
                })
            {
                return Err(rollback.abort(
                    &mut staging,
                    PublicationState::Partial {
                        completed_final_renames,
                        total_final_renames,
                    },
                    error.into(),
                    hooks,
                ));
            }
            completed_final_renames += 1;
        }
    }

    let mut finalized_snapshots = Vec::with_capacity(prepared.len());
    for (artifact, pinned) in prepared.iter().zip(pinned_snapshots) {
        match pinned.materialize(output, &artifact.names) {
            Ok(snapshot) => finalized_snapshots.push(snapshot),
            Err(error) => {
                return Err(rollback.abort(
                    &mut staging,
                    PublicationState::FinalsPublished {
                        final_renames: completed_final_renames,
                    },
                    error,
                    hooks,
                ));
            }
        }
    }

    let stale = old_owned
        .union(&orphaned)
        .filter(|kernel| !new_owned.contains(*kernel))
        .cloned()
        .collect::<BTreeSet<_>>();
    let (_, stale_failures) = invalidate_kernels(output, &stale, hooks);
    if !stale_failures.is_empty() {
        let mut error = rollback.abort(
            &mut staging,
            PublicationState::FinalsPublished {
                final_renames: completed_final_renames,
            },
            ownership_error("failed to remove stale producer artifacts"),
            hooks,
        );
        if let EmitError::Transaction(transaction) = &mut error {
            transaction
                .invalidation_failures
                .splice(0..0, stale_failures);
        }
        return Err(error);
    }

    if let Err(error) = commit_registry(
        output,
        &staging,
        &next_registry,
        completed_final_renames,
        hooks,
    ) {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::FinalsPublished {
                final_renames: completed_final_renames,
            },
            error,
            hooks,
        ));
    }

    let mut primary = fsync(&output.fd)
        .map_err(std::io::Error::from)
        .err()
        .map(EmitError::from);
    if let Err(error) = hooks.after_registry_commit()
        && primary.is_none()
    {
        primary = Some(error.into());
    }
    if let Err(error) = output.verify_path_identity()
        && primary.is_none()
    {
        primary = Some(error);
    }

    let mut cleanup_failures = staging.cleanup(hooks);
    if let Err(error) = fsync(&output.fd).map_err(std::io::Error::from) {
        cleanup_failures.push(FilesystemFailure {
            operation: "persist staging cleanup",
            entry: output.display_path.display().to_string(),
            error,
        });
    }
    if primary.is_some() || !cleanup_failures.is_empty() {
        let publication = if cleanup_failures.is_empty() {
            PublicationState::Committed {
                final_renames: completed_final_renames,
            }
        } else {
            PublicationState::CommittedWithCleanupFailure {
                final_renames: completed_final_renames,
            }
        };
        return Err(EmitError::Transaction(Box::new(ArtifactTransactionError {
            primary: primary.map(Box::new),
            cleanup_failures,
            invalidation_failures: Vec::new(),
            publication,
        })));
    }
    Ok(prepared
        .iter()
        .zip(finalized_snapshots)
        .map(|(artifact, (llvm_ir, hsaco))| DeviceArtifact {
            kernel_name: artifact.names.kernel.clone(),
            llvm_ir,
            hsaco,
        })
        .collect())
}

fn read_registry(output: &PinnedOutput) -> Result<OwnershipRegistry, EmitError> {
    let Some(bytes) = read_control_file(
        output,
        OWNERSHIP_FILE,
        "ownership registry",
        MAX_OWNERSHIP_BYTES,
    )?
    else {
        return Ok(OwnershipRegistry::default());
    };
    OwnershipRegistry::decode(&bytes)
}

fn read_attempt_registry(output: &PinnedOutput) -> Result<AttemptRegistry, EmitError> {
    recover_attempt_registry(output)?;
    let Some(bytes) = read_control_file(
        output,
        ATTEMPT_FILE,
        "build-attempt registry",
        MAX_ATTEMPT_BYTES,
    )?
    else {
        return Ok(AttemptRegistry::default());
    };
    AttemptRegistry::decode(&bytes).map_err(build_attempt_error)
}

fn recover_attempt_registry(output: &PinnedOutput) -> Result<(), EmitError> {
    let Some(bytes) = read_control_file(
        output,
        RECOVERY_ATTEMPT_FILE,
        "build-attempt recovery registry",
        MAX_ATTEMPT_BYTES,
    )?
    else {
        return Ok(());
    };
    AttemptRegistry::decode(&bytes).map_err(|error| {
        build_attempt_error(format!(
            "durable build-attempt recovery cannot be replayed: {error}"
        ))
    })?;
    output.verify_path_identity()?;
    renameat(&output.fd, RECOVERY_ATTEMPT_FILE, &output.fd, ATTEMPT_FILE)
        .map_err(std::io::Error::from)?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    Ok(())
}

fn read_control_file(
    output: &PinnedOutput,
    entry: &str,
    description: &str,
    maximum_bytes: usize,
) -> Result<Option<Vec<u8>>, EmitError> {
    let fd = match openat(
        &output.fd,
        entry,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let stat = fstat(&fd).map_err(std::io::Error::from)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_nlink != 1
        || stat.st_mode & 0o077 != 0
    {
        return Err(EmitError::InvalidArtifactDestination {
            path: output.display_path.join(entry),
            reason: format!("{description} is not a private single-link regular file"),
        });
    }
    let mut bytes = Vec::new();
    fs::File::from(fd)
        .take((maximum_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn prune_absent_ownership(
    output: &PinnedOutput,
    registry: &mut OwnershipRegistry,
) -> Result<bool, EmitError> {
    let mut absent = Vec::new();
    for (source, ownership) in &registry.producers {
        for kernel in &ownership.kernels {
            let artifact = ArtifactNames::new(kernel)?;
            let mut any_present = false;
            for entry in artifact.files() {
                match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(_) => any_present = true,
                    Err(error) if error == rustix::io::Errno::NOENT => {}
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
            }
            if !any_present {
                absent.push((source.clone(), kernel.clone()));
            }
        }
    }
    if absent.is_empty() {
        return Ok(false);
    }
    for (source, kernel) in absent {
        if let Some(ownership) = registry.producers.get_mut(&source) {
            ownership.kernels.remove(&kernel);
        }
    }
    registry
        .producers
        .retain(|_, ownership| !ownership.kernels.is_empty());
    Ok(true)
}

fn commit_registry_direct(
    output: &PinnedOutput,
    registry: &OwnershipRegistry,
) -> Result<(), EmitError> {
    let bytes = (!registry.producers.is_empty())
        .then(|| registry.encode())
        .transpose()?;
    commit_control_file_direct(
        output,
        OWNERSHIP_FILE,
        RECOVERY_OWNERSHIP_FILE,
        bytes.as_deref(),
    )
}

fn commit_attempt_registry_direct(
    output: &PinnedOutput,
    registry: &AttemptRegistry,
) -> Result<(), EmitError> {
    commit_attempt_registry_direct_with_hooks(output, registry, &mut NoControlCommitFaults)
}

fn commit_attempt_registry_direct_with_hooks(
    output: &PinnedOutput,
    registry: &AttemptRegistry,
    hooks: &mut impl ControlCommitHooks,
) -> Result<(), EmitError> {
    let bytes = registry.encode().map_err(build_attempt_error)?;
    commit_control_file_direct_with_hooks(
        output,
        ATTEMPT_FILE,
        RECOVERY_ATTEMPT_FILE,
        Some(&bytes),
        hooks,
    )
}

fn commit_control_file_direct(
    output: &PinnedOutput,
    final_entry: &str,
    recovery_entry: &str,
    bytes: Option<&[u8]>,
) -> Result<(), EmitError> {
    commit_control_file_direct_with_hooks(
        output,
        final_entry,
        recovery_entry,
        bytes,
        &mut NoControlCommitFaults,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ControlCommitPoint {
    CreateRecovery,
    WriteRecovery,
    SyncRecovery,
    SyncRecoveryName,
    RenameRecovery,
    SyncFinalName,
}

trait ControlCommitHooks {
    fn before(&mut self, _point: ControlCommitPoint) -> io::Result<()> {
        Ok(())
    }
}

struct NoControlCommitFaults;

impl ControlCommitHooks for NoControlCommitFaults {}

fn commit_control_file_direct_with_hooks(
    output: &PinnedOutput,
    final_entry: &str,
    recovery_entry: &str,
    bytes: Option<&[u8]>,
    hooks: &mut impl ControlCommitHooks,
) -> Result<(), EmitError> {
    let Some(bytes) = bytes else {
        match unlinkat(&output.fd, final_entry, AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
        return Ok(());
    };

    hooks.before(ControlCommitPoint::CreateRecovery)?;
    let fd = openat(
        &output.fd,
        recovery_entry,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(std::io::Error::from)?;
    let result = (|| {
        let mut file = fs::File::from(fd);
        hooks.before(ControlCommitPoint::WriteRecovery)?;
        file.write_all(bytes)?;
        hooks.before(ControlCommitPoint::SyncRecovery)?;
        file.sync_all()?;
        // The redo name must be durable before rename consumes it. A crash during the rename or
        // its directory sync then leaves either the new final entry or a replayable redo entry.
        hooks.before(ControlCommitPoint::SyncRecoveryName)?;
        fsync(&output.fd).map_err(std::io::Error::from)?;
        output.verify_path_identity()?;
        hooks.before(ControlCommitPoint::RenameRecovery)?;
        renameat(&output.fd, recovery_entry, &output.fd, final_entry)
            .map_err(std::io::Error::from)?;
        hooks.before(ControlCommitPoint::SyncFinalName)?;
        fsync(&output.fd).map_err(std::io::Error::from)?;
        Ok(())
    })();
    if result.is_err() {
        // Preserve even a partial redo entry as poison. Deleting it could expose an older
        // authorizing registry after a failed state transition.
        let _ = fsync(&output.fd);
    }
    result
}

fn stage_registry(
    staging: &StagingDirectory,
    registry: &OwnershipRegistry,
) -> Result<(), EmitError> {
    if registry.producers.is_empty() {
        match unlinkat(&staging.fd, STAGED_OWNERSHIP_FILE, AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        Ok(())
    } else {
        staging.replace(STAGED_OWNERSHIP_FILE, &registry.encode()?)
    }
}

fn commit_registry(
    output: &PinnedOutput,
    staging: &StagingDirectory,
    registry: &OwnershipRegistry,
    completed_final_renames: usize,
    hooks: &mut impl TransactionHooks,
) -> Result<(), EmitError> {
    if registry.producers.is_empty() {
        match unlinkat(&output.fd, OWNERSHIP_FILE, AtFlags::empty()) {
            Ok(()) => Ok(()),
            Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
            Err(error) => Err(std::io::Error::from(error).into()),
        }
    } else {
        hooks
            .before_rename(RenameKind::Ownership, completed_final_renames)
            .map_err(EmitError::from)?;
        renameat(
            &staging.fd,
            STAGED_OWNERSHIP_FILE,
            &output.fd,
            OWNERSHIP_FILE,
        )
        .map_err(std::io::Error::from)?;
        Ok(())
    }
}

struct RollbackContext<'a> {
    output: &'a PinnedOutput,
    original_registry: &'a OwnershipRegistry,
    producer: &'a ProducerIdentity,
    invalidation_set: &'a BTreeSet<String>,
    recovery_candidates: &'a BTreeSet<String>,
}

fn abort_without_staging(
    output: &PinnedOutput,
    original_registry: &OwnershipRegistry,
    producer: &ProducerIdentity,
    invalidation_set: &BTreeSet<String>,
    recovery_candidates: &BTreeSet<String>,
    primary: EmitError,
    hooks: &mut impl TransactionHooks,
) -> EmitError {
    let (failed_kernels, invalidation_failures) =
        invalidate_kernels(output, invalidation_set, hooks);
    let failed_owned = failed_kernels
        .intersection(recovery_candidates)
        .cloned()
        .collect();
    let mut recovery_registry = original_registry.clone();
    recovery_registry.set_owned(producer, failed_owned);
    let mut cleanup_failures = Vec::new();
    if let Err(error) = commit_registry_direct(output, &recovery_registry) {
        cleanup_failures.push(FilesystemFailure {
            operation: "reconcile ownership without staging",
            entry: OWNERSHIP_FILE.to_string(),
            error: io::Error::other(error.to_string()),
        });
    }
    if let Err(error) = fsync(&output.fd).map_err(std::io::Error::from) {
        cleanup_failures.push(FilesystemFailure {
            operation: "persist rollback without staging",
            entry: output.display_path.display().to_string(),
            error,
        });
    }
    EmitError::Transaction(Box::new(ArtifactTransactionError {
        primary: Some(Box::new(primary)),
        cleanup_failures,
        invalidation_failures,
        publication: PublicationState::NotStarted {
            total_final_renames: 0,
        },
    }))
}

impl RollbackContext<'_> {
    fn abort(
        &self,
        staging: &mut StagingDirectory,
        publication: PublicationState,
        primary: EmitError,
        hooks: &mut impl TransactionHooks,
    ) -> EmitError {
        let (failed_kernels, invalidation_failures) =
            invalidate_kernels(self.output, self.invalidation_set, hooks);
        let failed_owned = failed_kernels
            .intersection(self.recovery_candidates)
            .cloned()
            .collect();
        let mut recovery_registry = self.original_registry.clone();
        recovery_registry.set_owned(self.producer, failed_owned);
        let mut cleanup_failures = Vec::new();
        if let Err(error) = stage_registry(staging, &recovery_registry)
            .and_then(|()| commit_registry(self.output, staging, &recovery_registry, 0, hooks))
        {
            cleanup_failures.push(FilesystemFailure {
                operation: "reconcile ownership after failure",
                entry: OWNERSHIP_FILE.to_string(),
                error: io::Error::other(error.to_string()),
            });
        }
        cleanup_failures.extend(staging.cleanup(hooks));
        if let Err(error) = fsync(&self.output.fd).map_err(std::io::Error::from) {
            cleanup_failures.push(FilesystemFailure {
                operation: "persist transaction rollback",
                entry: self.output.display_path.display().to_string(),
                error,
            });
        }
        EmitError::Transaction(Box::new(ArtifactTransactionError {
            primary: Some(Box::new(primary)),
            cleanup_failures,
            invalidation_failures,
            publication,
        }))
    }
}

fn invalidate_kernels(
    output: &PinnedOutput,
    kernels: &BTreeSet<String>,
    hooks: &mut impl TransactionHooks,
) -> (BTreeSet<String>, Vec<FilesystemFailure>) {
    let mut failed_kernels = BTreeSet::new();
    let mut failures = Vec::new();
    for kernel in kernels {
        let artifact = match ArtifactNames::new(kernel) {
            Ok(artifact) => artifact,
            Err(error) => {
                failed_kernels.insert(kernel.clone());
                failures.push(FilesystemFailure {
                    operation: "validate owned artifact name",
                    entry: kernel.clone(),
                    error: io::Error::other(error.to_string()),
                });
                continue;
            }
        };
        for entry in artifact.files() {
            let result = hooks.before_invalidate(entry).and_then(|()| {
                unlinkat(&output.fd, entry, AtFlags::empty()).map_err(std::io::Error::from)
            });
            match result {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    failed_kernels.insert(kernel.clone());
                    failures.push(FilesystemFailure {
                        operation: "invalidate artifact",
                        entry: entry.to_string(),
                        error,
                    });
                }
            }
        }
    }
    (failed_kernels, failures)
}

fn invalidate_producer_ownership(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    hooks: &mut impl TransactionHooks,
) -> Result<(), EmitError> {
    let mut registry = read_registry(output)?;
    let pruned = prune_absent_ownership(output, &mut registry)?;
    let owned = registry.owned_by(producer);
    let (failed_kernels, invalidation_failures) = invalidate_kernels(output, &owned, hooks);
    let failed_owned = failed_kernels
        .intersection(&owned)
        .cloned()
        .collect::<BTreeSet<_>>();
    registry.set_owned(producer, failed_owned);

    let mut cleanup_failures = Vec::new();
    if (pruned || !owned.is_empty())
        && let Err(error) = commit_registry_direct(output, &registry)
    {
        cleanup_failures.push(FilesystemFailure {
            operation: "reconcile ownership during generation invalidation",
            entry: OWNERSHIP_FILE.to_string(),
            error: io::Error::other(error.to_string()),
        });
    }
    if invalidation_failures.is_empty() && cleanup_failures.is_empty() {
        return Ok(());
    }
    Err(EmitError::Transaction(Box::new(ArtifactTransactionError {
        primary: Some(Box::new(build_attempt_error(
            "could not invalidate the producer's prior artifact generation",
        ))),
        cleanup_failures,
        invalidation_failures,
        publication: PublicationState::NotStarted {
            total_final_renames: 0,
        },
    })))
}

fn fail_build_attempt_locked(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    hooks: &mut impl TransactionHooks,
) -> Result<(), EmitError> {
    let state_result = (|| {
        let mut attempts = read_attempt_registry(output)?;
        let record = attempts
            .record_exact(&producer.stable_source, attempt)
            .map_err(build_attempt_error)?;
        if record.crate_name != producer.crate_name {
            return Err(build_attempt_error(
                "build attempt token does not match the producer",
            ));
        }
        attempts
            .mark_failed(&producer.stable_source, attempt)
            .map_err(build_attempt_error)?;
        commit_attempt_registry_direct(output, &attempts)
    })();
    let invalidation_result = invalidate_producer_ownership(output, producer, hooks);
    match (state_result, invalidation_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) | (Err(error), Ok(())) => Err(error),
        (Err(state_error), Err(EmitError::Transaction(mut transaction))) => {
            transaction.primary = Some(Box::new(state_error));
            Err(EmitError::Transaction(transaction))
        }
        (Err(state_error), Err(invalidation_error)) => {
            Err(EmitError::Transaction(Box::new(ArtifactTransactionError {
                primary: Some(Box::new(state_error)),
                cleanup_failures: vec![FilesystemFailure {
                    operation: "prepare failed-attempt invalidation",
                    entry: output.display_path.display().to_string(),
                    error: io::Error::other(invalidation_error.to_string()),
                }],
                invalidation_failures: Vec::new(),
                publication: PublicationState::NotStarted {
                    total_final_renames: 0,
                },
            })))
        }
    }
}

fn claim_attempt_for_termination_locked(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), EmitError> {
    claim_attempt_for_termination_locked_with_hooks(
        output,
        producer,
        attempt,
        &mut NoControlCommitFaults,
    )
}

fn claim_attempt_for_termination_locked_with_hooks(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    hooks: &mut impl ControlCommitHooks,
) -> Result<(), EmitError> {
    let mut attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)?;
    if record.crate_name != producer.crate_name {
        return Err(build_attempt_error(
            "build attempt token does not match the producer",
        ));
    }
    match record.phase {
        AttemptPhase::Building => {
            attempts
                .claim_backend(&producer.stable_source, attempt)
                .map_err(build_attempt_error)?;
            commit_attempt_registry_direct_with_hooks(output, &attempts, hooks)
        }
        AttemptPhase::BackendClaimed | AttemptPhase::Failed => Ok(()),
        AttemptPhase::Invalidating | AttemptPhase::Completed => Err(build_attempt_error(
            "build attempt cannot be terminated in its current phase",
        )),
    }
}

fn prepare_backend_attempt_locked(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    supplied_attempt: Option<BuildAttempt>,
) -> Result<(BuildAttempt, bool), EmitError> {
    let mut attempts = read_attempt_registry(output)?;
    if let Some(attempt) = supplied_attempt {
        if attempt.session() == BuildSession::DIRECT {
            return Err(build_attempt_error(
                "the direct compiler token cannot authorize a managed backend",
            ));
        }
        let record = attempts
            .authorize_backend(&producer.stable_source, attempt)
            .map_err(build_attempt_error)?;
        if record.crate_name != producer.crate_name {
            return Err(build_attempt_error(
                "build attempt crate name does not match the producer",
            ));
        }
        attempts
            .claim_backend(&producer.stable_source, attempt)
            .map_err(build_attempt_error)?;
        commit_attempt_registry_direct(output, &attempts)?;
        return Ok((attempt, true));
    }

    if let Some(record) = attempts.record(&producer.stable_source)
        && record.session != BuildSession::DIRECT
    {
        return Err(build_attempt_error(
            "a cargo-managed build attempt is active for this producer",
        ));
    }
    let attempt = attempts
        .allocate_direct(&producer.stable_source, &producer.crate_name)
        .map_err(build_attempt_error)?;
    commit_attempt_registry_direct(output, &attempts)?;
    attempts
        .transition_building(&producer.stable_source, attempt)
        .map_err(build_attempt_error)?;
    commit_attempt_registry_direct(output, &attempts)?;
    attempts
        .claim_backend(&producer.stable_source, attempt)
        .map_err(build_attempt_error)?;
    commit_attempt_registry_direct(output, &attempts)?;
    Ok((attempt, false))
}

fn finish_backend_attempt_locked(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    externally_managed: bool,
    result: Result<Vec<DeviceArtifact>, EmitError>,
    hooks: &mut impl TransactionHooks,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    if !externally_managed {
        let publication_committed = matches!(
            &result,
            Err(EmitError::Transaction(transaction)) if transaction.publication().is_committed()
        );
        let state_result = (|| {
            let mut attempts = read_attempt_registry(output)?;
            if result.is_ok() || publication_committed {
                attempts
                    .record_legacy_backend_receipt(&producer.stable_source, attempt)
                    .map_err(build_attempt_error)?;
                attempts
                    .mark_completed(&producer.stable_source, attempt)
                    .map_err(build_attempt_error)?;
            } else {
                attempts
                    .mark_failed(&producer.stable_source, attempt)
                    .map_err(build_attempt_error)?;
            }
            commit_attempt_registry_direct(output, &attempts)
        })();
        return match result {
            Ok(value) => match state_result {
                Ok(()) => Ok(value),
                Err(primary) => Err(committed_attempt_error(primary, value.len() * 3)),
            },
            Err(primary) => match state_result {
                Ok(()) => Err(primary),
                Err(secondary) => Err(combine_attempt_errors(primary, secondary, output)),
            },
        };
    }

    match result {
        Ok(value) => {
            let mut attempts = match read_attempt_registry(output) {
                Ok(attempts) => attempts,
                Err(primary) => {
                    let primary = committed_attempt_error(primary, value.len() * 3);
                    return Err(fail_after_backend_error(
                        output, producer, attempt, primary, hooks,
                    ));
                }
            };
            let record = match attempts.record_exact(&producer.stable_source, attempt) {
                Ok(record) => record,
                Err(error) => {
                    let primary =
                        committed_attempt_error(build_attempt_error(error), value.len() * 3);
                    return Err(fail_after_backend_error(
                        output, producer, attempt, primary, hooks,
                    ));
                }
            };
            if record.crate_name != producer.crate_name {
                let primary = committed_attempt_error(
                    build_attempt_error(
                        "build attempt crate name changed before backend completion",
                    ),
                    value.len() * 3,
                );
                return Err(fail_after_backend_error(
                    output, producer, attempt, primary, hooks,
                ));
            }
            let state_result = attempts
                .record_legacy_backend_receipt(&producer.stable_source, attempt)
                .map_err(build_attempt_error)
                .and_then(|()| commit_attempt_registry_direct(output, &attempts));
            if let Err(primary) = state_result {
                let primary = committed_attempt_error(primary, value.len() * 3);
                return Err(fail_after_backend_error(
                    output, producer, attempt, primary, hooks,
                ));
            }
            Ok(value)
        }
        Err(primary) => Err(fail_after_backend_error(
            output, producer, attempt, primary, hooks,
        )),
    }
}

fn committed_attempt_error(primary: EmitError, final_renames: usize) -> EmitError {
    EmitError::Transaction(Box::new(ArtifactTransactionError {
        primary: Some(Box::new(primary)),
        cleanup_failures: Vec::new(),
        invalidation_failures: Vec::new(),
        publication: PublicationState::Committed { final_renames },
    }))
}

fn fail_after_backend_error(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    primary: EmitError,
    hooks: &mut impl TransactionHooks,
) -> EmitError {
    match fail_build_attempt_locked(output, producer, attempt, hooks) {
        Ok(()) => primary,
        Err(secondary) => combine_attempt_errors(primary, secondary, output),
    }
}

fn combine_attempt_errors(
    primary: EmitError,
    secondary: EmitError,
    output: &PinnedOutput,
) -> EmitError {
    if let EmitError::Transaction(mut primary_transaction) = primary {
        match secondary {
            EmitError::Transaction(mut secondary_transaction) => {
                if let Some(secondary_primary) = secondary_transaction.primary.take() {
                    primary_transaction
                        .cleanup_failures
                        .push(FilesystemFailure {
                            operation: "persist failed build attempt",
                            entry: ATTEMPT_FILE.to_string(),
                            error: io::Error::other(secondary_primary.to_string()),
                        });
                }
                primary_transaction
                    .cleanup_failures
                    .append(&mut secondary_transaction.cleanup_failures);
                primary_transaction
                    .invalidation_failures
                    .append(&mut secondary_transaction.invalidation_failures);
            }
            secondary => primary_transaction
                .cleanup_failures
                .push(FilesystemFailure {
                    operation: "persist failed build attempt",
                    entry: output.display_path.display().to_string(),
                    error: io::Error::other(secondary.to_string()),
                }),
        }
        return EmitError::Transaction(primary_transaction);
    }

    match secondary {
        EmitError::Transaction(mut transaction) => {
            if let Some(secondary_primary) = transaction.primary.take() {
                transaction.cleanup_failures.insert(
                    0,
                    FilesystemFailure {
                        operation: "persist failed build attempt",
                        entry: ATTEMPT_FILE.to_string(),
                        error: io::Error::other(secondary_primary.to_string()),
                    },
                );
            }
            transaction.primary = Some(Box::new(primary));
            EmitError::Transaction(transaction)
        }
        secondary => EmitError::Transaction(Box::new(ArtifactTransactionError {
            primary: Some(Box::new(primary)),
            cleanup_failures: vec![FilesystemFailure {
                operation: "persist failed build attempt",
                entry: output.display_path.display().to_string(),
                error: io::Error::other(secondary.to_string()),
            }],
            invalidation_failures: Vec::new(),
            publication: PublicationState::NotStarted {
                total_final_renames: 0,
            },
        })),
    }
}

fn validate_owned_destinations(
    output: &PinnedOutput,
    artifact: &ArtifactNames,
) -> Result<(), EmitError> {
    for entry in artifact.files() {
        match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat)
                if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                    || FileType::from_raw_mode(stat.st_mode) == FileType::Symlink => {}
            Ok(_) => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(entry),
                    reason: "owned destination is not a regular file or symlink".to_string(),
                });
            }
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Ok(())
}

fn validate_staged_artifacts(
    staging: &StagingDirectory,
    artifact: &ArtifactNames,
    output: &PinnedOutput,
) -> Result<(), EmitError> {
    for entry in artifact.files() {
        let fd = match openat(
            &staging.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(_) => {
                return Err(EmitError::MissingStagedArtifact {
                    path: output.display_path.join(&staging.name).join(entry),
                });
            }
        };
        let stat = fstat(&fd).map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(EmitError::MissingStagedArtifact {
                path: output.display_path.join(&staging.name).join(entry),
            });
        }
        fsync(&fd).map_err(std::io::Error::from)?;
    }
    Ok(())
}

fn validate_simple_name(name: &str, label: &str) -> Result<(), EmitError> {
    if name.is_empty()
        || name.len() > MAX_ARTIFACT_NAME_BYTES
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(EmitError::InvalidProducer {
            reason: format!(
                "{label} must contain 1 to {MAX_ARTIFACT_NAME_BYTES} ASCII letters, digits, or underscores"
            ),
        });
    }
    Ok(())
}

fn validate_artifact_name(kernel_name: &str) -> Result<(), EmitError> {
    let display_name = if kernel_name.len() <= MAX_ARTIFACT_NAME_BYTES {
        kernel_name.to_string()
    } else {
        format!("<{}-byte name>", kernel_name.len())
    };
    if kernel_name.is_empty() {
        return Err(EmitError::InvalidArtifactName {
            kernel: display_name,
            reason: "name is empty".to_string(),
        });
    }
    if kernel_name.len() > MAX_ARTIFACT_NAME_BYTES {
        return Err(EmitError::InvalidArtifactName {
            kernel: display_name,
            reason: format!("name exceeds {MAX_ARTIFACT_NAME_BYTES} bytes"),
        });
    }
    let mut bytes = kernel_name.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(EmitError::InvalidArtifactName {
            kernel: display_name,
            reason: "name must be an ASCII identifier".to_string(),
        });
    }
    Ok(())
}

fn build_attempt_error(error: impl fmt::Display) -> EmitError {
    EmitError::BuildAttempt {
        reason: error.to_string(),
    }
}
