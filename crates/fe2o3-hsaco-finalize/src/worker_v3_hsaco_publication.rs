//! Typed durable-publication bridge for native strict-V3 finalized HSACO.

use std::{error::Error, fmt, path::Path, sync::Arc};

use crate::worker_v3_finalized_schema::{DescriptorSchema, FinalizedOwner, FinalizedRef};
use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationErrorV3,
    AttemptScopedHsacoPublicationResultV3, BuildAttempt, CanonicalLinkRequestIdentityV1,
    CompilerExecutionSubjectErrorV1, DurableCurrentLinkPublicationLeaseV1,
    DurableLinkPublicationPlanV1, DurablePublishedHsacoClaimV3, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, InertCompilerExecutionSubjectV1, KernelSetIdentityV1,
    LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1,
    ProducerIdentity, RecoveredWorkerV3PublicationIntentV1, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    VerifiedWorkerV3PublicationAuthorityV1, WorkerV3FinalizerReplayAttachmentsV1,
    WorkerV3PublicationBindingErrorV1, WorkerV3PublicationBindingV1,
    WorkerV3PublicationIntentErrorV1, WorkerV3PublicationIntentOutcomeV1,
    WorkerV3PublicationIntentRecordV1, persist_worker_v3_publication_intent_v1,
    producer_package_identity_v1, publish_exact_hsaco_evidence_for_attempt_v3,
    recover_worker_v3_publication_intent_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use sha2::{Digest, Sha256};

#[path = "nominal_worker_publication_v3.rs"]
mod nominal;
use nominal::RecoveredPublicationRef;
pub use nominal::{
    PreparedNominalWorkerPublicationV3, PublishedNominalWorkerHsacoV3,
    RecoveredNominalWorkerPublicationV3, persist_prepared_nominal_worker_publication_v3,
    prepare_nominal_worker_publication_v3, publish_recovered_nominal_worker_hsaco_v3,
    recover_nominal_worker_publication_v3,
};

use crate::{
    ContentIdentityV1, FinalizedProtectedWorkerV3HsacoIdentityV1,
    InspectedProtectedWorkerV3HsacoIdentityV1, LinkPlanIdentityV1,
    PreparedFinalizedProtectedWorkerV3HsacoV1, PreparedProtectedWorkerV3CompactFinalizerReplayV2,
    ProtectedCompilerHandoffBindingIdentityV3, ProtectedCompilerHandoffBindingV3,
    ProtectedFirstBuildWorkerV3Error, ProtectedFirstBuildWorkerV3IdentityV1,
    ProtectedWorkerV3CompactFinalizerReplayErrorV1,
    ProtectedWorkerV3CompactFinalizerReplayIdentityV2, ProtectedWorkerV3CompactFinalizerReplayV2,
    WorkerDerivationEvidenceV1, WorkerInputV1, WorkerMeasurementV1, WorkerProtocolError,
    WorkerRequestConstructionError, finalize_protected_worker_v3_hsaco_v1,
    first_build_worker_v3::recover_inert_protected_first_build_worker_v3_evidence_v1,
    inspect_protected_worker_v3_hsaco_v1,
    request_construction::decode_compiler_module_handoff_v2,
    worker_v3_compact_finalizer_replay::{
        OwnedProtectedWorkerV3CompactFinalizerReplayPartsV2,
        ProtectedWorkerV3CompactFinalizerReplayPartsV2,
    },
};

const KERNEL_SET_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-KERNEL-SET/V1\0";
const TARGET_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-TARGET/V1\0";
const REQUEST_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-REQUEST/V1\0";
const WORKER_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-WORKER/V1\0";
const RESPONSE_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-RESPONSE/V1\0";
const FINALIZATION_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-FINALIZATION/V1\0";
const PUBLICATION_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-ATOMIC-PUBLICATION/V1\0";
const UPSTREAM_DOMAIN_V1: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-UPSTREAM/V1\0";
const REVALIDATED_FINALIZER_DERIVATION_DOMAIN_V1: &[u8] =
    b"FE2O3/PROTECTED-WORKER-V3-REVALIDATED-FINALIZER-DERIVATION/V1\0";

/// Stable identity of one independently reconstructed Worker V3 finalizer derivation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RevalidatedProtectedWorkerV3FinalizerDerivationIdentityV1([u8; 32]);

impl RevalidatedProtectedWorkerV3FinalizerDerivationIdentityV1 {
    /// Returns the domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Move-only, authority-free custody of an independently reconstructed finalizer lineage.
///
/// This compact owner binds the durable transcript, strict bootstrap and replay requests, exact
/// compiler module, measured worker, canonical link plan, linked/optimized LLVM modules, generated
/// object, ordered native-link inputs, in-process LLD policy, raw HSACO, descriptor finalization,
/// and finalized HSACO. It records exact byte and policy custody only; it does not prove semantic
/// refinement from LLVM to machine code and grants no compiler, publication, load, or launch
/// authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RevalidatedProtectedWorkerV3FinalizerDerivationV1;
///
/// fn cannot_duplicate(value: RevalidatedProtectedWorkerV3FinalizerDerivationV1) {
///     let _duplicate = value.clone();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
    identity: RevalidatedProtectedWorkerV3FinalizerDerivationIdentityV1,
    transcript: ProtectedWorkerV3CompactFinalizerReplayIdentityV2,
    source: ProtectedFirstBuildWorkerV3IdentityV1,
    binding: ProtectedCompilerHandoffBindingIdentityV3,
    worker: WorkerMeasurementV1,
    bootstrap_request_id: [u8; 32],
    bootstrap_request_identity: [u8; 32],
    replay_request_id: [u8; 32],
    replay_request_identity: [u8; 32],
    compiler_module: ContentIdentityV1,
    link_plan: LinkPlanIdentityV1,
    derivation: WorkerDerivationEvidenceV1,
    raw_hsaco: ContentIdentityV1,
    finalization: FinalizedProtectedWorkerV3HsacoIdentityV1,
    finalized_hsaco: ContentIdentityV1,
    descriptor_schema: u16,
}

impl RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
    /// Descriptor wire version independently replayed from exact ABI receipt bytes.
    pub const fn descriptor_schema_version(&self) -> u16 {
        self.descriptor_schema
    }
    /// Returns the identity of every retained finalizer-custody axis.
    pub const fn identity(&self) -> RevalidatedProtectedWorkerV3FinalizerDerivationIdentityV1 {
        self.identity
    }

    /// Returns the exact compact transcript identity.
    pub const fn transcript_identity(&self) -> ProtectedWorkerV3CompactFinalizerReplayIdentityV2 {
        self.transcript
    }

    /// Returns the strict bootstrap/replay source-evidence identity.
    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV3IdentityV1 {
        self.source
    }

    /// Returns the complete compiler-handoff binding identity.
    pub const fn binding_identity(&self) -> ProtectedCompilerHandoffBindingIdentityV3 {
        self.binding
    }

    /// Returns the exact measured worker declaration.
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    /// Returns the bootstrap request identifier.
    pub const fn bootstrap_request_id(&self) -> &[u8; 32] {
        &self.bootstrap_request_id
    }

    /// Returns the canonical bootstrap request identity.
    pub const fn bootstrap_request_identity(&self) -> &[u8; 32] {
        &self.bootstrap_request_identity
    }

    /// Returns the exact-output replay request identifier.
    pub const fn replay_request_id(&self) -> &[u8; 32] {
        &self.replay_request_id
    }

    /// Returns the canonical exact-output replay request identity.
    pub const fn replay_request_identity(&self) -> &[u8; 32] {
        &self.replay_request_identity
    }

    /// Returns the compiler-module content identity sent to the worker.
    pub const fn compiler_module_identity(&self) -> ContentIdentityV1 {
        self.compiler_module
    }

    /// Returns the canonical multi-input link-plan identity.
    pub const fn link_plan_identity(&self) -> LinkPlanIdentityV1 {
        self.link_plan
    }

    /// Returns the independently decoded LLVM/object/LLD derivation evidence.
    pub const fn derivation_evidence(&self) -> &WorkerDerivationEvidenceV1 {
        &self.derivation
    }

    /// Returns the raw worker-produced HSACO identity.
    pub const fn raw_hsaco_identity(&self) -> ContentIdentityV1 {
        self.raw_hsaco
    }

    /// Returns the canonical descriptor-finalization identity.
    pub const fn finalization_identity(&self) -> FinalizedProtectedWorkerV3HsacoIdentityV1 {
        self.finalization
    }

    /// Returns the finalized HSACO identity.
    pub const fn finalized_hsaco_identity(&self) -> ContentIdentityV1 {
        self.finalized_hsaco
    }

    /// Exact custody does not prove semantic refinement from LLVM to machine code.
    pub const fn proves_llvm_to_machine_semantic_refinement(&self) -> bool {
        false
    }

    /// Reports that this structural evidence grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that this structural evidence grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that this structural evidence grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that this structural evidence grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Copyable identity view of one internally derived strict-V3 publication intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedProtectedWorkerV3HsacoPublicationIntentV1 {
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    finalization: FinalizedProtectedWorkerV3HsacoIdentityV1,
    source: ProtectedFirstBuildWorkerV3IdentityV1,
    binding: ProtectedCompilerHandoffBindingIdentityV3,
    raw_inspection: InspectedProtectedWorkerV3HsacoIdentityV1,
    raw_output: ContentIdentityV1,
    finalized_output: ContentIdentityV1,
}

impl SealedProtectedWorkerV3HsacoPublicationIntentV1 {
    pub const fn durable_plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    pub const fn upstream_evidence(self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream
    }

    pub const fn finalization_identity(self) -> FinalizedProtectedWorkerV3HsacoIdentityV1 {
        self.finalization
    }

    pub const fn source_evidence_identity(self) -> ProtectedFirstBuildWorkerV3IdentityV1 {
        self.source
    }

    pub const fn binding_identity(self) -> ProtectedCompilerHandoffBindingIdentityV3 {
        self.binding
    }

    pub const fn raw_inspection_identity(self) -> InspectedProtectedWorkerV3HsacoIdentityV1 {
        self.raw_inspection
    }

    pub const fn raw_output_identity(self) -> ContentIdentityV1 {
        self.raw_output
    }

    pub const fn finalized_output_identity(self) -> ContentIdentityV1 {
        self.finalized_output
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

/// Move-only V3 restart owner with an internally derived durable plan.
pub struct PreparedProtectedWorkerV3HsacoPublicationV1 {
    producer_package: PackageIdentityV1,
    intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
    replay: PreparedProtectedWorkerV3CompactFinalizerReplayV2,
}

impl fmt::Debug for PreparedProtectedWorkerV3HsacoPublicationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedProtectedWorkerV3HsacoPublicationV1")
            .field("attempt", &self.intent.plan.attempt())
            .field("finalization", &self.intent.finalization)
            .field("source", &self.intent.source)
            .field("binding", &self.intent.binding)
            .field("replay", &self.replay)
            .finish()
    }
}

impl PreparedProtectedWorkerV3HsacoPublicationV1 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.intent.plan.attempt()
    }

    pub const fn publication_intent(&self) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        self.intent
    }

    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        self.replay.exact_finalized_hsaco()
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

/// Fully revalidated, inert V3 lineage recovered from durable restart storage.
#[derive(Debug)]
pub struct RecoveredProtectedWorkerV3HsacoPublicationV1 {
    outcome: WorkerV3PublicationIntentOutcomeV1,
    record: WorkerV3PublicationIntentRecordV1,
    finalized: PreparedFinalizedProtectedWorkerV3HsacoV1,
    intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
}

/// Move-only production result retaining both replayed lineage and completed publication state.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PublishedProtectedWorkerV3HsacoV1;
///
/// fn cannot_clone_published_v3(value: PublishedProtectedWorkerV3HsacoV1) {
///     let _duplicate = value.clone();
/// }
/// ```
#[derive(Debug)]
pub struct PublishedProtectedWorkerV3HsacoV1 {
    recovered: RecoveredProtectedWorkerV3HsacoPublicationV1,
    publication: AttemptScopedHsacoPublicationResultV3,
}

impl PublishedProtectedWorkerV3HsacoV1 {
    pub const fn recovered_evidence(&self) -> &RecoveredProtectedWorkerV3HsacoPublicationV1 {
        &self.recovered
    }

    pub const fn publication_result(&self) -> &AttemptScopedHsacoPublicationResultV3 {
        &self.publication
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
        self.publication.published_claim()
    }

    /// Reconstructs the exact authority-free compiler occurrence subject retained by this result.
    pub fn compiler_execution_subject_v1(
        &self,
    ) -> Result<InertCompilerExecutionSubjectV1, CompilerExecutionSubjectErrorV1> {
        self.recovered.compiler_execution_subject_v1()
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Transfers exact publication custody into the future V3 load-envelope layer.
    ///
    /// The compact replay components are regenerated from the independently reconstructed
    /// finalizer owner. The returned lease pins the exact published files but still grants no load
    /// or launch authority. No V1/V2 projection occurs at this boundary.
    pub fn into_load_envelope_parts_v1(
        self,
    ) -> Result<PublishedProtectedWorkerV3LoadEnvelopePartsV1, WorkerV3HsacoPublicationErrorV1>
    {
        let Self {
            recovered,
            publication,
        } = self;
        let storage_record = recovered.record;
        let claim = publication.published_claim().clone();
        let current_lease = publication.into_current_lease();
        let replay =
            crate::prepare_protected_worker_v3_compact_finalizer_replay_v2(recovered.finalized)?
                .into_parts();
        Ok(PublishedProtectedWorkerV3LoadEnvelopePartsV1 {
            replay,
            storage_record,
            claim,
            current_lease,
        })
    }
}

/// Unique owners transferred from one completed V3 publication into load-envelope preparation.
///
/// This value cannot be constructed outside this crate. Its serialized fields remain inert; the
/// retained current-publication lease is the only occurrence-sensitive component and still grants
/// no HSA load or launch authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PublishedProtectedWorkerV3LoadEnvelopePartsV1;
///
/// fn cannot_extract_private_lease(value: PublishedProtectedWorkerV3LoadEnvelopePartsV1) {
///     let PublishedProtectedWorkerV3LoadEnvelopePartsV1 { current_lease, .. } = value;
/// }
/// ```
pub struct PublishedProtectedWorkerV3LoadEnvelopePartsV1 {
    replay: ProtectedWorkerV3CompactFinalizerReplayPartsV2,
    storage_record: WorkerV3PublicationIntentRecordV1,
    claim: DurablePublishedHsacoClaimV3,
    current_lease: DurableCurrentLinkPublicationLeaseV1,
}

impl fmt::Debug for PublishedProtectedWorkerV3LoadEnvelopePartsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishedProtectedWorkerV3LoadEnvelopePartsV1")
            .field("replay", &self.replay)
            .field("storage_record", &self.storage_record.identity())
            .field("claim", &self.claim)
            .field("current_lease", &self.current_lease)
            .finish()
    }
}

impl PublishedProtectedWorkerV3LoadEnvelopePartsV1 {
    /// Transfers all exact inert bytes and the non-clone publication lease to the envelope owner.
    pub fn into_parts(
        self,
    ) -> (
        ProtectedWorkerV3CompactFinalizerReplayPartsV2,
        WorkerV3PublicationIntentRecordV1,
        DurablePublishedHsacoClaimV3,
        DurableCurrentLinkPublicationLeaseV1,
    ) {
        (
            self.replay,
            self.storage_record,
            self.claim,
            self.current_lease,
        )
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl RecoveredProtectedWorkerV3HsacoPublicationV1 {
    pub const fn outcome(&self) -> WorkerV3PublicationIntentOutcomeV1 {
        self.outcome
    }

    pub const fn storage_record(&self) -> WorkerV3PublicationIntentRecordV1 {
        self.record
    }

    pub const fn publication_intent(&self) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        self.intent
    }

    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        self.finalized.exact_finalized_bytes()
    }

    pub const fn finalized_evidence(&self) -> &PreparedFinalizedProtectedWorkerV3HsacoV1 {
        &self.finalized
    }

    /// Reconstructs the exact authority-free compiler occurrence before HSACO publication.
    pub fn compiler_execution_subject_v1(
        &self,
    ) -> Result<InertCompilerExecutionSubjectV1, CompilerExecutionSubjectErrorV1> {
        InertCompilerExecutionSubjectV1::from_replay_evidence(
            self.finalized.attempt(),
            self.finalized.handoff_slot(),
            self.finalized.transaction_identity(),
            self.finalized.outer_handoff(),
        )
    }

    /// Derives the exact transaction-owned binding required to complete V3 publication.
    ///
    /// The compiler closure is supplied by the authenticated Cargo boundary. Every other axis is
    /// taken from this independently replayed restart owner and cannot be mixed with a different
    /// publication-intent record or finalizer lineage.
    pub fn publication_binding(
        &self,
        compiler_closure: CompilerClosureV2,
    ) -> Result<WorkerV3PublicationBindingV1, WorkerV3HsacoPublicationErrorV1> {
        RecoveredPublicationRef::V1(self).publication_binding(compiler_closure)
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
#[non_exhaustive]
pub enum WorkerV3HsacoPublicationErrorV1 {
    DescriptorSchemaMismatch,
    NominalArtifact(crate::NominalFinalizationErrorV3<std::convert::Infallible>),
    NominalFinalization(crate::NominalWorkerFinalizationErrorV3<std::convert::Infallible>),
    ProducerIdentityMismatch,
    CompilerClosureMismatch,
    MissingExactFinalizerDerivation,
    RawOutputMismatch,
    FinalizedOutputMismatch,
    TranscriptFinalizationMismatch,
    TranscriptSourceMismatch,
    DurablePlanMismatch,
    ProviderCountMismatch,
    ProviderIdentityMismatch { index: usize },
    CompactReplay(ProtectedWorkerV3CompactFinalizerReplayErrorV1),
    Storage(WorkerV3PublicationIntentErrorV1),
    PublicationBinding(WorkerV3PublicationBindingErrorV1),
    Transaction(AttemptScopedHsacoPublicationErrorV3),
    OuterHandoff(fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3),
    Binding(crate::ProtectedCompilerHandoffBindingErrorV3),
    Request(WorkerRequestConstructionError),
    Protocol(WorkerProtocolError),
    FirstBuild(ProtectedFirstBuildWorkerV3Error),
    Inspection(crate::WorkerV3HsacoInspectionError),
    Finalization(crate::WorkerV3HsacoFinalizationError),
    FinalizedHsaco(crate::FinalizationError),
    LinkPlan(crate::LinkPlanError),
    AllocationFailed { component: &'static str },
}

impl fmt::Display for WorkerV3HsacoPublicationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescriptorSchemaMismatch => {
                formatter.write_str("descriptor schema does not match the typed finalizer boundary")
            }
            Self::NominalArtifact(error) => error.fmt(formatter),
            Self::NominalFinalization(error) => error.fmt(formatter),
            Self::ProducerIdentityMismatch => {
                formatter.write_str("V3 publication producer differs from the prepared producer")
            }
            Self::CompilerClosureMismatch => formatter
                .write_str("V3 publication compiler closure differs from recovered handoff"),
            Self::MissingExactFinalizerDerivation => formatter.write_str(
                "legacy Worker V3 finalizer replay lacks exact LLVM/object/LLD derivation custody",
            ),
            Self::RawOutputMismatch => {
                formatter.write_str("recovered raw HSACO differs from strict V3 replay evidence")
            }
            Self::FinalizedOutputMismatch => formatter
                .write_str("recovered finalized HSACO differs from canonical V3 finalization"),
            Self::TranscriptFinalizationMismatch => formatter
                .write_str("compact replay finalization identity was not independently reproduced"),
            Self::TranscriptSourceMismatch => formatter
                .write_str("compact replay source identity was not independently reproduced"),
            Self::DurablePlanMismatch => {
                formatter.write_str("durable V3 publication plan was not independently reproduced")
            }
            Self::ProviderCountMismatch => {
                formatter.write_str("stored provider count differs from compact replay metadata")
            }
            Self::ProviderIdentityMismatch { index } => {
                write!(
                    formatter,
                    "stored provider {index} differs from compact replay metadata"
                )
            }
            Self::CompactReplay(error) => error.fmt(formatter),
            Self::Storage(error) => error.fmt(formatter),
            Self::PublicationBinding(error) => error.fmt(formatter),
            Self::Transaction(error) => error.fmt(formatter),
            Self::OuterHandoff(error) => error.fmt(formatter),
            Self::Binding(error) => error.fmt(formatter),
            Self::Request(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
            Self::FirstBuild(error) => error.fmt(formatter),
            Self::Inspection(error) => error.fmt(formatter),
            Self::Finalization(error) => error.fmt(formatter),
            Self::FinalizedHsaco(error) => error.fmt(formatter),
            Self::LinkPlan(error) => error.fmt(formatter),
            Self::AllocationFailed { component } => {
                write!(formatter, "could not allocate recovered V3 {component}")
            }
        }
    }
}

impl Error for WorkerV3HsacoPublicationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NominalArtifact(error) => Some(error),
            Self::NominalFinalization(error) => Some(error),
            Self::CompactReplay(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::PublicationBinding(error) => Some(error),
            Self::Transaction(error) => Some(error),
            Self::OuterHandoff(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Request(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::FirstBuild(error) => Some(error),
            Self::Inspection(error) => Some(error),
            Self::Finalization(error) => Some(error),
            Self::FinalizedHsaco(error) => Some(error),
            Self::LinkPlan(error) => Some(error),
            _ => None,
        }
    }
}

macro_rules! error_conversion {
    ($source:ty, $variant:ident) => {
        impl From<$source> for WorkerV3HsacoPublicationErrorV1 {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

error_conversion!(
    ProtectedWorkerV3CompactFinalizerReplayErrorV1,
    CompactReplay
);
error_conversion!(WorkerV3PublicationIntentErrorV1, Storage);
error_conversion!(WorkerV3PublicationBindingErrorV1, PublicationBinding);
error_conversion!(AttemptScopedHsacoPublicationErrorV3, Transaction);
error_conversion!(
    fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3,
    OuterHandoff
);
error_conversion!(crate::ProtectedCompilerHandoffBindingErrorV3, Binding);
error_conversion!(WorkerRequestConstructionError, Request);
error_conversion!(WorkerProtocolError, Protocol);
error_conversion!(ProtectedFirstBuildWorkerV3Error, FirstBuild);
error_conversion!(crate::WorkerV3HsacoInspectionError, Inspection);
error_conversion!(crate::WorkerV3HsacoFinalizationError, Finalization);
error_conversion!(crate::FinalizationError, FinalizedHsaco);
error_conversion!(crate::LinkPlanError, LinkPlan);

/// Consumes finalized V3 evidence and derives its durable plan and compact V2 replay internally.
pub fn prepare_protected_worker_v3_hsaco_publication_v1(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<PreparedProtectedWorkerV3HsacoPublicationV1, WorkerV3HsacoPublicationErrorV1> {
    prepare_versioned_publication(producer, FinalizedOwner::V1(finalized))
}

fn prepare_versioned_publication(
    producer: &ProducerIdentity,
    finalized: FinalizedOwner,
) -> Result<PreparedProtectedWorkerV3HsacoPublicationV1, WorkerV3HsacoPublicationErrorV1> {
    let producer_package = producer_package_identity_v1(producer);
    let intent = derive_publication_intent(producer_package, finalized.view())?;
    let replay =
        crate::worker_v3_compact_finalizer_replay::prepare_versioned_compact_finalizer_replay(
            finalized,
        )?;
    Ok(PreparedProtectedWorkerV3HsacoPublicationV1 {
        producer_package,
        intent,
        replay,
    })
}

/// Persists one prepared V3 owner and immediately runs the same exact validator as restart.
pub fn persist_prepared_protected_worker_v3_hsaco_publication_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: PreparedProtectedWorkerV3HsacoPublicationV1,
) -> Result<RecoveredProtectedWorkerV3HsacoPublicationV1, WorkerV3HsacoPublicationErrorV1> {
    let recovered = persist_versioned_publication(output_dir, producer, prepared)?;
    validate_recovered_publication(producer, recovered)
}

fn persist_versioned_publication(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: PreparedProtectedWorkerV3HsacoPublicationV1,
) -> Result<RecoveredWorkerV3PublicationIntentV1, WorkerV3HsacoPublicationErrorV1> {
    if producer_package_identity_v1(producer) != prepared.producer_package {
        return Err(WorkerV3HsacoPublicationErrorV1::ProducerIdentityMismatch);
    }
    let attempt = prepared.intent.plan.attempt();
    let plan = prepared.intent.plan;
    let OwnedProtectedWorkerV3CompactFinalizerReplayPartsV2 {
        outer_handoff,
        external_provider_payloads,
        transcript,
        finalized_hsaco,
    } = prepared.replay.into_storage_parts();
    let attachments = WorkerV3FinalizerReplayAttachmentsV1::new(
        outer_handoff,
        external_provider_payloads,
        transcript,
    )
    .map_err(WorkerV3PublicationIntentErrorV1::Codec)?;
    Ok(persist_worker_v3_publication_intent_v1(
        output_dir,
        producer,
        attempt,
        plan,
        attachments,
        finalized_hsaco,
    )?)
}

/// Recovers one durable V3 occurrence and independently reproduces its complete finalizer lineage.
pub fn recover_protected_worker_v3_hsaco_publication_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredProtectedWorkerV3HsacoPublicationV1, WorkerV3HsacoPublicationErrorV1> {
    let recovered = recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)?;
    validate_recovered_publication(producer, recovered)
}

/// Independently reconstructs exact finalizer custody from borrowed durable-envelope components.
///
/// The returned value is compact and move-only. This operation uses bounded transient copies for
/// external providers while rebuilding the exact strict Worker V3 requests and responses; no
/// provider, compiler-module, raw-HSACO, or finalized-HSACO payload is retained in the result.
pub fn revalidate_protected_worker_v3_finalizer_derivation_v1<'payload, I>(
    attempt: BuildAttempt,
    outer_handoff: &[u8],
    external_provider_payloads: I,
    transcript: &[u8],
    exact_finalized_hsaco: &[u8],
) -> Result<RevalidatedProtectedWorkerV3FinalizerDerivationV1, WorkerV3HsacoPublicationErrorV1>
where
    I: IntoIterator<Item = &'payload [u8]>,
    I::IntoIter: ExactSizeIterator,
{
    let transcript = ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(transcript)?;
    let outer = InertSemanticCompilerModuleHandoffV3::decode(outer_handoff)?;
    let payloads = external_provider_payloads.into_iter();
    let mut borrowed = try_vec(payloads.len(), "borrowed provider inputs")?;
    for payload in payloads {
        borrowed.push(BorrowedProviderPayloadV1(payload));
    }
    Ok(validate_finalizer_replay_components(
        attempt,
        outer,
        borrowed,
        transcript,
        exact_finalized_hsaco,
    )?
    .derivation)
}

pub(crate) fn revalidate_protected_worker_v3_finalizer_for_structural_archive_v1<'payload, I>(
    attempt: BuildAttempt,
    outer_handoff: &[u8],
    external_provider_payloads: I,
    transcript: &[u8],
    exact_finalized_hsaco: &[u8],
) -> Result<PreparedFinalizedProtectedWorkerV3HsacoV1, WorkerV3HsacoPublicationErrorV1>
where
    I: IntoIterator<Item = &'payload [u8]>,
    I::IntoIter: ExactSizeIterator,
{
    let transcript = ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(transcript)?;
    let outer = InertSemanticCompilerModuleHandoffV3::decode(outer_handoff)?;
    let payloads = external_provider_payloads.into_iter();
    let mut borrowed = try_vec(payloads.len(), "structural archive provider inputs")?;
    for payload in payloads {
        borrowed.push(BorrowedProviderPayloadV1(payload));
    }
    Ok(validate_finalizer_replay_components(
        attempt,
        outer,
        borrowed,
        transcript,
        exact_finalized_hsaco,
    )?
    .finalized
    .into_v1()?)
}

/// Completes the production V3 publication path from one independently replayed restart owner.
///
/// This is the production entry point. The lower-level artifact-transaction V3 API independently
/// requires matching durable restart storage under the publication lock, but only this facade
/// reconstructs and authenticates the strict finalizer transcript before publication.
pub fn publish_recovered_protected_worker_v3_hsaco_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    compiler_closure: CompilerClosureV2,
    recovered: RecoveredProtectedWorkerV3HsacoPublicationV1,
) -> Result<PublishedProtectedWorkerV3HsacoV1, WorkerV3HsacoPublicationErrorV1> {
    let publication = publish_recovered_versioned(
        output_dir,
        producer,
        compiler_closure,
        RecoveredPublicationRef::V1(&recovered),
    )?;
    Ok(PublishedProtectedWorkerV3HsacoV1 {
        recovered,
        publication,
    })
}

#[allow(
    unsafe_code,
    reason = "one audited authority bridge follows complete schema-explicit strict-finalizer replay"
)]
fn publish_recovered_versioned(
    output_dir: &Path,
    producer: &ProducerIdentity,
    compiler_closure: CompilerClosureV2,
    recovered: RecoveredPublicationRef<'_>,
) -> Result<AttemptScopedHsacoPublicationResultV3, WorkerV3HsacoPublicationErrorV1> {
    let intent = recovered.intent();
    let binding = recovered.publication_binding(compiler_closure)?;
    // SAFETY: Both closed variants retain independently reconstructed finalizer
    // custody. Shared replay checked exact schema, every stored input and binding
    // axis. Each typed facade retains its owner alongside the publication result.
    let authority = unsafe {
        VerifiedWorkerV3PublicationAuthorityV1::from_authenticated_finalizer_replay_unchecked(
            binding,
        )
    };
    Ok(publish_exact_hsaco_evidence_for_attempt_v3(
        output_dir,
        producer,
        intent.durable_plan().attempt(),
        intent.durable_plan(),
        intent.upstream_evidence(),
        authority,
        recovered.finalized().bytes(),
    )?)
}

fn validate_recovered_publication(
    producer: &ProducerIdentity,
    recovered: RecoveredWorkerV3PublicationIntentV1,
) -> Result<RecoveredProtectedWorkerV3HsacoPublicationV1, WorkerV3HsacoPublicationErrorV1> {
    let ValidatedRecoveredPublication {
        outcome,
        record,
        finalized,
        intent,
    } = validate_recovered_versioned(producer, recovered)?;
    Ok(RecoveredProtectedWorkerV3HsacoPublicationV1 {
        outcome,
        record,
        finalized: finalized.into_v1()?,
        intent,
    })
}

struct ValidatedRecoveredPublication {
    outcome: WorkerV3PublicationIntentOutcomeV1,
    record: WorkerV3PublicationIntentRecordV1,
    finalized: FinalizedOwner,
    intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
}

fn validate_recovered_versioned(
    producer: &ProducerIdentity,
    recovered: RecoveredWorkerV3PublicationIntentV1,
) -> Result<ValidatedRecoveredPublication, WorkerV3HsacoPublicationErrorV1> {
    let outcome = recovered.outcome();
    let (record, attachments, exact_finalized_hsaco) = recovered.into_parts();
    let (outer_handoff, provider_payloads, transcript_bytes) = attachments.into_parts();
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&transcript_bytes)?;
    let outer = InertSemanticCompilerModuleHandoffV3::decode_shared_vec(Arc::new(outer_handoff))?;
    let validated = validate_finalizer_replay_components(
        record.attempt(),
        outer,
        provider_payloads,
        transcript,
        &exact_finalized_hsaco,
    )?;
    let finalized = validated.finalized;
    let intent =
        derive_publication_intent(producer_package_identity_v1(producer), finalized.view())?;
    if intent.plan != record.plan() {
        return Err(WorkerV3HsacoPublicationErrorV1::DurablePlanMismatch);
    }
    Ok(ValidatedRecoveredPublication {
        outcome,
        record,
        finalized,
        intent,
    })
}

struct ValidatedFinalizerReplayComponentsV1 {
    finalized: FinalizedOwner,
    derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
}

trait FinalizerProviderPayloadV1 {
    fn as_bytes(&self) -> &[u8];

    fn into_owned(self) -> Result<Vec<u8>, WorkerV3HsacoPublicationErrorV1>;
}

impl FinalizerProviderPayloadV1 for Vec<u8> {
    fn as_bytes(&self) -> &[u8] {
        self
    }

    fn into_owned(self) -> Result<Vec<u8>, WorkerV3HsacoPublicationErrorV1> {
        Ok(self)
    }
}

struct BorrowedProviderPayloadV1<'payload>(&'payload [u8]);

impl FinalizerProviderPayloadV1 for BorrowedProviderPayloadV1<'_> {
    fn as_bytes(&self) -> &[u8] {
        self.0
    }

    fn into_owned(self) -> Result<Vec<u8>, WorkerV3HsacoPublicationErrorV1> {
        try_copy_bytes(self.0, "borrowed provider payload")
    }
}

fn validate_finalizer_replay_components<P: FinalizerProviderPayloadV1>(
    attempt: BuildAttempt,
    outer: InertSemanticCompilerModuleHandoffV3,
    provider_payloads: Vec<P>,
    transcript: ProtectedWorkerV3CompactFinalizerReplayV2,
    exact_finalized_hsaco: &[u8],
) -> Result<ValidatedFinalizerReplayComponentsV1, WorkerV3HsacoPublicationErrorV1> {
    if !transcript.retains_derivation_metadata() {
        return Err(WorkerV3HsacoPublicationErrorV1::MissingExactFinalizerDerivation);
    }
    let binding = ProtectedCompilerHandoffBindingV3::from_replay_parts(
        &outer,
        attempt,
        transcript.handoff_slot(),
        transcript.transaction_identity(),
    )?;
    let transcript_identity = transcript.identity();
    let replay = transcript.replay_view();
    if replay.external_providers.len() != provider_payloads.len() {
        return Err(WorkerV3HsacoPublicationErrorV1::ProviderCountMismatch);
    }
    let mut providers = try_vec(provider_payloads.len(), "provider inputs")?;
    for (index, (reference, payload)) in replay
        .external_providers
        .iter()
        .zip(provider_payloads)
        .enumerate()
    {
        if !reference.identity.matches(payload.as_bytes()) {
            return Err(WorkerV3HsacoPublicationErrorV1::ProviderIdentityMismatch { index });
        }
        let input = WorkerInputV1::new(reference.kind, payload.into_owned()?)?;
        if input.identity() != reference.identity {
            return Err(WorkerV3HsacoPublicationErrorV1::ProviderIdentityMismatch { index });
        }
        providers.push(input);
    }

    let decoded = decode_compiler_module_handoff_v2(outer.module_handoff().canonical_bytes())
        .map_err(WorkerRequestConstructionError::CompilerModuleHandoff)?;
    let schema = DescriptorSchema::from_abi(outer.capsule().receipts().abi().canonical_preimage())?;
    let raw_hsaco = schema.derive_raw(exact_finalized_hsaco)?;
    let crate::worker_finalizer_replay_engine::ReconstructedWorkerExchanges {
        plan,
        bootstrap_request_bytes,
        bootstrap_response,
        replay_request_bytes,
        replay_response,
    } = crate::worker_finalizer_replay_engine::reconstruct_worker_exchanges(
        (&binding).into(),
        &decoded,
        providers,
        &replay,
        &raw_hsaco,
    )?;
    let source = recover_inert_protected_first_build_worker_v3_evidence_v1(
        binding,
        outer,
        replay.worker.clone(),
        replay.execution_limits,
        plan,
        bootstrap_request_bytes,
        bootstrap_response,
        replay_request_bytes,
        replay_response,
    )?;
    if source.identity().as_bytes() != transcript.source_evidence_identity() {
        return Err(WorkerV3HsacoPublicationErrorV1::TranscriptSourceMismatch);
    }
    let finalized = match schema {
        DescriptorSchema::V1 => FinalizedOwner::V1(finalize_protected_worker_v3_hsaco_v1(
            inspect_protected_worker_v3_hsaco_v1(source)?,
        )?),
        DescriptorSchema::NominalV3 => FinalizedOwner::NominalV3(
            crate::finalize_protected_worker_nominal_hsaco_v3(
                source,
                crate::NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                &mut |_| Ok::<_, std::convert::Infallible>(()),
            )
            .map_err(WorkerV3HsacoPublicationErrorV1::NominalFinalization)?,
        ),
    };
    let view = finalized.view();
    if view.raw().exact_bytes() != raw_hsaco {
        return Err(WorkerV3HsacoPublicationErrorV1::RawOutputMismatch);
    }
    if view.identity().as_bytes() != transcript.expected_finalization_identity() {
        return Err(WorkerV3HsacoPublicationErrorV1::TranscriptFinalizationMismatch);
    }
    if view.bytes() != exact_finalized_hsaco {
        return Err(WorkerV3HsacoPublicationErrorV1::FinalizedOutputMismatch);
    }
    let derivation = derive_revalidated_finalizer_derivation(transcript_identity, view);
    Ok(ValidatedFinalizerReplayComponentsV1 {
        finalized,
        derivation,
    })
}

fn derive_revalidated_finalizer_derivation(
    transcript: ProtectedWorkerV3CompactFinalizerReplayIdentityV2,
    finalized: FinalizedRef<'_>,
) -> RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
    let raw = finalized.raw();
    let source = raw.source_evidence();
    let worker = source.worker_measurement().clone();
    let bootstrap = source.bootstrap().response();
    let replay = source.exact_replay().response();
    let compiler_module =
        ContentIdentityV1::calculate(raw.outer_handoff().module_handoff().module_bytes());
    let derivation = source.derivation_evidence().clone();
    let raw_hsaco = raw.linked_output_identity();
    debug_assert_eq!(derivation.hsaco(), raw_hsaco);
    let finalization = finalized.identity();
    let finalized_hsaco = finalized.output_identity();
    let identity = RevalidatedProtectedWorkerV3FinalizerDerivationIdentityV1(hash_identity(
        REVALIDATED_FINALIZER_DERIVATION_DOMAIN_V1,
        |hash| {
            hash.update(transcript.as_bytes());
            hash.update(source.identity().as_bytes());
            hash.update(source.binding().identity().as_bytes());
            hash_content_identity(hash, worker.executable());
            hash_text(hash, worker.worker_build_identity());
            hash_text(hash, worker.llvm_build_identity());
            hash.update(bootstrap.request_id());
            hash.update(bootstrap.request_identity());
            hash.update(replay.request_id());
            hash.update(replay.request_identity());
            hash_content_identity(hash, compiler_module);
            hash.update(source.plan().identity().as_bytes());
            hash.update(derivation.evidence_identity());
            hash_content_identity(hash, derivation.linked_module());
            hash_content_identity(hash, derivation.optimized_module());
            hash_content_identity(hash, derivation.generated_object());
            hash.update((derivation.native_link_inputs().len() as u64).to_le_bytes());
            for input in derivation.native_link_inputs() {
                hash.update([input.source() as u8]);
                hash_content_identity(hash, input.content());
            }
            hash.update(derivation.lld_invocation_identity());
            hash_content_identity(hash, raw_hsaco);
            hash.update(finalization.as_bytes());
            hash_content_identity(hash, finalized_hsaco);
        },
    ));
    RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        identity,
        transcript,
        source: source.identity(),
        binding: source.binding().identity(),
        worker,
        bootstrap_request_id: *bootstrap.request_id(),
        bootstrap_request_identity: *bootstrap.request_identity(),
        replay_request_id: *replay.request_id(),
        replay_request_identity: *replay.request_identity(),
        compiler_module,
        link_plan: source.plan().identity(),
        derivation,
        raw_hsaco,
        finalization,
        finalized_hsaco,
        descriptor_schema: match finalized.schema() {
            DescriptorSchema::V1 => 1,
            DescriptorSchema::NominalV3 => 3,
        },
    }
}

fn derive_publication_intent(
    producer_package: PackageIdentityV1,
    finalized: FinalizedRef<'_>,
) -> Result<SealedProtectedWorkerV3HsacoPublicationIntentV1, WorkerV3HsacoPublicationErrorV1> {
    if !finalized.output_identity().matches(finalized.bytes()) {
        return Err(WorkerV3HsacoPublicationErrorV1::FinalizedOutputMismatch);
    }
    let raw_bytes = finalized.schema().derive_raw(finalized.bytes())?;
    let raw = finalized.raw();
    if !raw.linked_output_identity().matches(&raw_bytes) {
        return Err(WorkerV3HsacoPublicationErrorV1::RawOutputMismatch);
    }
    let manifest = raw
        .outer_handoff()
        .module_handoff()
        .symbol_manifest()
        .identity();
    let kernel_set = KernelSetIdentityV1::from_bytes(hash_identity(KERNEL_SET_DOMAIN_V1, |hash| {
        hash.update(manifest.sha256());
        hash.update(manifest.byte_len().to_le_bytes());
        hash.update(raw.outer_handoff_identity().sha256());
    }));
    let target_text = raw.target().to_string();
    let target = TargetIdentityV1::from_bytes(hash_identity(TARGET_DOMAIN_V1, |hash| {
        hash_blob(hash, target_text.as_bytes());
        hash.update([raw.code_object_version().number()]);
        hash.update(raw.policy().identity().as_bytes());
    }));
    let scope = LinkPublicationScopeV1::new(producer_package, kernel_set, target);
    let expectation = raw.binding_expectation();
    let request =
        CanonicalLinkRequestIdentityV1::from_bytes(hash_identity(REQUEST_DOMAIN_V1, |hash| {
            hash_attempt(hash, raw.attempt());
            hash.update([raw.handoff_slot() as u8]);
            hash.update(raw.transaction_identity().as_bytes());
            hash.update(raw.outer_handoff_identity().sha256());
            hash.update(raw.binding_identity().as_bytes());
            hash.update(raw.source_evidence_identity().as_bytes());
            hash.update(raw.identity().as_bytes());
            hash.update(raw.link_plan_identity().as_bytes());
            hash.update(raw.policy().identity().as_bytes());
            hash.update(expectation.invocation_digest());
        }));
    let measurement = raw.worker_measurement();
    let worker = PinnedWorkerIdentityV1::from_bytes(hash_identity(WORKER_DOMAIN_V1, |hash| {
        hash_content(hash, measurement.executable());
        hash_blob(hash, measurement.worker_build_identity().as_bytes());
        hash_blob(hash, measurement.llvm_build_identity().as_bytes());
    }));
    let response =
        ValidatedResponseIdentityV1::from_bytes(hash_identity(RESPONSE_DOMAIN_V1, |hash| {
            hash.update(raw.source_evidence_identity().as_bytes());
            hash.update(raw.identity().as_bytes());
            hash_content(hash, raw.linked_output_identity());
        }));
    let linked_output = LinkedOutputIdentityV1::from_bytes(*raw.linked_output_identity().sha256());
    let finalization =
        FinalizationIdentityV1::from_bytes(hash_identity(FINALIZATION_DOMAIN_V1, |hash| {
            hash.update(finalized.identity().as_bytes());
            hash.update(finalized.canonical_digest().as_bytes());
            hash_content(hash, finalized.descriptor_identity());
            hash_content(hash, raw.linked_output_identity());
            hash_content(hash, finalized.output_identity());
        }));
    let finalized_output =
        FinalizedOutputIdentityV1::from_bytes(*finalized.output_identity().sha256());
    let publication =
        AtomicPublicationIdentityV1::from_bytes(hash_identity(PUBLICATION_DOMAIN_V1, |hash| {
            hash_attempt(hash, raw.attempt());
            hash.update(producer_package.as_bytes());
            hash.update(kernel_set.as_bytes());
            hash.update(target.as_bytes());
            hash.update(request.as_bytes());
            hash.update(worker.as_bytes());
            hash.update(response.as_bytes());
            hash.update(linked_output.as_bytes());
            hash.update(finalization.as_bytes());
            hash.update(finalized_output.as_bytes());
        }));
    let plan = DurableLinkPublicationPlanV1::new(
        raw.attempt(),
        scope,
        request,
        worker,
        response,
        linked_output,
        finalization,
        finalized_output,
        publication,
    );
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(hash_identity(
        UPSTREAM_DOMAIN_V1,
        |hash| {
            hash.update(finalized.identity().as_bytes());
            hash.update(raw.source_evidence_identity().as_bytes());
            hash.update(raw.binding_identity().as_bytes());
            hash.update(finalization.as_bytes());
        },
    ));
    Ok(SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        plan,
        upstream,
        finalization: finalized.identity(),
        source: raw.source_evidence_identity(),
        binding: raw.binding_identity(),
        raw_inspection: raw.identity(),
        raw_output: raw.linked_output_identity(),
        finalized_output: finalized.output_identity(),
    })
}

pub(crate) fn try_vec<T>(
    capacity: usize,
    component: &'static str,
) -> Result<Vec<T>, WorkerV3HsacoPublicationErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| WorkerV3HsacoPublicationErrorV1::AllocationFailed { component })?;
    Ok(values)
}

pub(crate) fn try_copy_bytes(
    bytes: &[u8],
    component: &'static str,
) -> Result<Vec<u8>, WorkerV3HsacoPublicationErrorV1> {
    let mut value = try_vec(bytes.len(), component)?;
    value.extend_from_slice(bytes);
    Ok(value)
}

fn hash_identity(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    update(&mut hash);
    hash.finalize().into()
}

fn hash_attempt(hash: &mut Sha256, attempt: BuildAttempt) {
    hash.update(attempt.generation().to_le_bytes());
    hash.update(attempt.session().as_bytes());
    hash.update(attempt.invocation().as_bytes());
}

fn hash_blob(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash_blob(hash, value.as_bytes());
}

fn hash_content_identity(hash: &mut Sha256, identity: ContentIdentityV1) {
    hash.update(identity.sha256());
    hash.update(identity.byte_len().to_le_bytes());
}

fn hash_content(hash: &mut Sha256, identity: ContentIdentityV1) {
    hash.update(identity.sha256());
    hash.update(identity.byte_len().to_le_bytes());
}
