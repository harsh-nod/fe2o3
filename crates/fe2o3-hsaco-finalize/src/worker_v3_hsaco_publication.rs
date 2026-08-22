//! Typed durable-publication bridge for native strict-V3 finalized HSACO.

use std::{error::Error, fmt, path::Path, sync::Arc};

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PinnedWorkerIdentityV1, ProducerIdentity, RecoveredWorkerV3PublicationIntentV1,
    TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    WorkerV3FinalizerReplayAttachmentsV1, WorkerV3PublicationIntentErrorV1,
    WorkerV3PublicationIntentOutcomeV1, WorkerV3PublicationIntentRecordV1,
    persist_worker_v3_publication_intent_v1, producer_package_identity_v1,
    recover_worker_v3_publication_intent_v1,
};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FinalizedProtectedWorkerV3HsacoIdentityV1,
    InspectedProtectedRawWorkerV3HsacoIdentityV1, LinkInputKindClosureV1, LinkInputV1,
    LinkOutputV1, MultiInputLinkPlanV1, PreparedFinalizedProtectedWorkerV3HsacoV1,
    PreparedProtectedWorkerV3CompactFinalizerReplayV2, ProtectedCompilerHandoffBindingIdentityV3,
    ProtectedCompilerHandoffBindingV3, ProtectedFirstBuildWorkerV3Error,
    ProtectedFirstBuildWorkerV3IdentityV1, ProtectedWorkerV3CompactFinalizerReplayErrorV1,
    ProtectedWorkerV3CompactFinalizerReplayV2, ProvenanceNodeV1, WorkerInputV1,
    WorkerOutputConstraintsV1, WorkerProtocolError, WorkerRequestConstructionError,
    derive_unfinalized_hsaco_from_finalized_v1, finalize_inspected_protected_worker_v3_hsaco_v1,
    first_build_worker_v3::recover_inert_protected_first_build_worker_v3_evidence_v1,
    inspect_protected_production_v1_worker_v3_raw_hsaco_v1,
    request_construction::{
        CompilerHandoffRequestBindingV2, construct_first_build_worker_request_v2_from_decoded,
        construct_plan_worker_request_v2_from_decoded, decode_compiler_module_handoff_v2,
        decode_link_options,
    },
    worker_protocol_v2::reconstruct_complete_worker_response_v2,
    worker_v3_compact_finalizer_replay::OwnedProtectedWorkerV3CompactFinalizerReplayPartsV2,
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

/// Copyable identity view of one internally derived strict-V3 publication intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedProtectedWorkerV3HsacoPublicationIntentV1 {
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    finalization: FinalizedProtectedWorkerV3HsacoIdentityV1,
    source: ProtectedFirstBuildWorkerV3IdentityV1,
    binding: ProtectedCompilerHandoffBindingIdentityV3,
    raw_inspection: InspectedProtectedRawWorkerV3HsacoIdentityV1,
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

    pub const fn raw_inspection_identity(self) -> InspectedProtectedRawWorkerV3HsacoIdentityV1 {
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
    ProducerIdentityMismatch,
    RawOutputMismatch,
    FinalizedOutputMismatch,
    TranscriptFinalizationMismatch,
    TranscriptSourceMismatch,
    DurablePlanMismatch,
    ProviderCountMismatch,
    ProviderIdentityMismatch { index: usize },
    CompactReplay(ProtectedWorkerV3CompactFinalizerReplayErrorV1),
    Storage(WorkerV3PublicationIntentErrorV1),
    OuterHandoff(fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3),
    Binding(crate::ProtectedCompilerHandoffBindingErrorV3),
    Request(WorkerRequestConstructionError),
    Protocol(WorkerProtocolError),
    FirstBuild(ProtectedFirstBuildWorkerV3Error),
    Inspection(crate::WorkerV2RawHsacoInspectionError),
    Finalization(crate::WorkerV2HsacoFinalizationError),
    FinalizedHsaco(crate::FinalizationError),
    LinkPlan(crate::LinkPlanError),
    AllocationFailed { component: &'static str },
}

impl fmt::Display for WorkerV3HsacoPublicationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProducerIdentityMismatch => {
                formatter.write_str("V3 publication producer differs from the prepared producer")
            }
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
            Self::CompactReplay(error) => Some(error),
            Self::Storage(error) => Some(error),
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
error_conversion!(
    fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3,
    OuterHandoff
);
error_conversion!(crate::ProtectedCompilerHandoffBindingErrorV3, Binding);
error_conversion!(WorkerRequestConstructionError, Request);
error_conversion!(WorkerProtocolError, Protocol);
error_conversion!(ProtectedFirstBuildWorkerV3Error, FirstBuild);
error_conversion!(crate::WorkerV2RawHsacoInspectionError, Inspection);
error_conversion!(crate::WorkerV2HsacoFinalizationError, Finalization);
error_conversion!(crate::FinalizationError, FinalizedHsaco);
error_conversion!(crate::LinkPlanError, LinkPlan);

/// Consumes finalized V3 evidence and derives its durable plan and compact V2 replay internally.
pub fn prepare_protected_worker_v3_hsaco_publication_v1(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<PreparedProtectedWorkerV3HsacoPublicationV1, WorkerV3HsacoPublicationErrorV1> {
    let producer_package = producer_package_identity_v1(producer);
    let intent = derive_publication_intent(producer_package, &finalized)?;
    let replay = crate::prepare_protected_worker_v3_compact_finalizer_replay_v2(finalized)?;
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
    let recovered = persist_worker_v3_publication_intent_v1(
        output_dir,
        producer,
        attempt,
        plan,
        attachments,
        finalized_hsaco,
    )?;
    validate_recovered_publication(producer, recovered)
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

fn validate_recovered_publication(
    producer: &ProducerIdentity,
    recovered: RecoveredWorkerV3PublicationIntentV1,
) -> Result<RecoveredProtectedWorkerV3HsacoPublicationV1, WorkerV3HsacoPublicationErrorV1> {
    let outcome = recovered.outcome();
    let (record, attachments, exact_finalized_hsaco) = recovered.into_parts();
    let (outer_handoff, provider_payloads, transcript_bytes) = attachments.into_parts();
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&transcript_bytes)?;
    let outer = InertSemanticCompilerModuleHandoffV3::decode_shared_vec(Arc::new(outer_handoff))?;
    let binding = ProtectedCompilerHandoffBindingV3::from_replay_parts(
        &outer,
        record.attempt(),
        transcript.handoff_slot(),
        transcript.transaction_identity(),
    )?;
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
        if !reference.identity.matches(&payload) {
            return Err(WorkerV3HsacoPublicationErrorV1::ProviderIdentityMismatch { index });
        }
        let input = WorkerInputV1::new(reference.kind, payload)?;
        if input.identity() != reference.identity {
            return Err(WorkerV3HsacoPublicationErrorV1::ProviderIdentityMismatch { index });
        }
        providers.push(input);
    }

    let decoded = decode_compiler_module_handoff_v2(outer.module_handoff().canonical_bytes())
        .map_err(WorkerRequestConstructionError::CompilerModuleHandoff)?;
    let (_, worker_options) = decode_link_options(replay.link_options)?;
    let bootstrap_output = WorkerOutputConstraintsV1::new(replay.bootstrap_output_bound)?;
    let bootstrap = construct_first_build_worker_request_v2_from_decoded(
        CompilerHandoffRequestBindingV2::ProtectedV3(&binding),
        replay.worker,
        &decoded,
        providers.clone(),
        worker_options,
        bootstrap_output,
    )?;
    let bootstrap_request_bytes = try_copy_bytes(
        bootstrap.sealed_request().canonical_bytes(),
        "bootstrap request wire",
    )?;
    let raw_hsaco = derive_unfinalized_hsaco_from_finalized_v1(&exact_finalized_hsaco)?;
    let raw_identity = ContentIdentityV1::calculate(&raw_hsaco);
    let plan = derive_link_plan(&decoded, &providers, replay.link_options, raw_identity)?;
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, plan_inputs_with_kinds(&decoded, &providers)?)?;
    let replay_output = WorkerOutputConstraintsV1::new(raw_identity.byte_len())?;
    let replay_request = construct_plan_worker_request_v2_from_decoded(
        CompilerHandoffRequestBindingV2::ProtectedV3(&binding),
        &plan,
        replay.worker,
        &decoded,
        providers,
        &input_kinds,
        replay_output,
    )?;
    let replay_request_bytes = try_copy_bytes(
        replay_request.sealed_request().canonical_bytes(),
        "replay request wire",
    )?;
    let bootstrap_response = reconstruct_complete_worker_response_v2(
        bootstrap.sealed_request(),
        &raw_hsaco,
        replay.bootstrap_metadata,
    )?;
    let replay_response = reconstruct_complete_worker_response_v2(
        replay_request.sealed_request(),
        &raw_hsaco,
        replay.replay_metadata,
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
    let inspected = inspect_protected_production_v1_worker_v3_raw_hsaco_v1(source)?;
    if inspected.exact_bytes() != raw_hsaco {
        return Err(WorkerV3HsacoPublicationErrorV1::RawOutputMismatch);
    }
    let finalized = finalize_inspected_protected_worker_v3_hsaco_v1(inspected)?;
    if finalized.identity().as_bytes() != transcript.expected_finalization_identity() {
        return Err(WorkerV3HsacoPublicationErrorV1::TranscriptFinalizationMismatch);
    }
    if finalized.exact_finalized_bytes() != exact_finalized_hsaco {
        return Err(WorkerV3HsacoPublicationErrorV1::FinalizedOutputMismatch);
    }
    let intent = derive_publication_intent(producer_package_identity_v1(producer), &finalized)?;
    if intent.plan != record.plan() {
        return Err(WorkerV3HsacoPublicationErrorV1::DurablePlanMismatch);
    }
    Ok(RecoveredProtectedWorkerV3HsacoPublicationV1 {
        outcome,
        record,
        finalized,
        intent,
    })
}

fn derive_link_plan(
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    providers: &[WorkerInputV1],
    options: &[crate::LinkOptionV1],
    output_identity: ContentIdentityV1,
) -> Result<MultiInputLinkPlanV1, WorkerV3HsacoPublicationErrorV1> {
    let mut inputs = try_vec(providers.len() + 1, "link inputs")?;
    inputs.extend_from_slice(providers);
    inputs.push(WorkerInputV1::new(
        decoded.compiler_module_kind(),
        try_copy_bytes(decoded.compiler_module_bytes(), "compiler module")?,
    )?);
    inputs.sort_by_key(|input| (input.identity(), input.kind()));
    let mut link_inputs = try_vec(inputs.len(), "link plan inputs")?;
    for input in &inputs {
        link_inputs.push(LinkInputV1::new(input.identity(), decoded.target()));
    }
    let mut provenance = try_vec(link_inputs.len() + 1, "link provenance")?;
    for input in &link_inputs {
        provenance.push(ProvenanceNodeV1::new(input.identity(), vec![])?);
    }
    let mut output_parents = try_vec(link_inputs.len(), "output provenance")?;
    for input in &link_inputs {
        output_parents.push(input.identity());
    }
    provenance.push(ProvenanceNodeV1::new(output_identity, output_parents)?);
    let mut canonical_options = try_vec(options.len(), "link options")?;
    canonical_options.extend_from_slice(options);
    Ok(MultiInputLinkPlanV1::canonicalized(
        decoded.target(),
        link_inputs,
        canonical_options,
        LinkOutputV1::new(output_identity, decoded.target()),
        provenance,
    )?)
}

fn plan_inputs_with_kinds(
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    providers: &[WorkerInputV1],
) -> Result<Vec<crate::WorkerInputKindV1>, WorkerV3HsacoPublicationErrorV1> {
    let mut inputs = try_vec(providers.len() + 1, "input-kind closure")?;
    inputs.extend(
        providers
            .iter()
            .map(|input| (input.identity(), input.kind())),
    );
    let compiler_identity = ContentIdentityV1::calculate(decoded.compiler_module_bytes());
    inputs.push((compiler_identity, decoded.compiler_module_kind()));
    inputs.sort_by_key(|input| *input);
    let mut kinds = try_vec(inputs.len(), "input-kind values")?;
    for (_, kind) in inputs {
        kinds.push(kind);
    }
    Ok(kinds)
}

fn derive_publication_intent(
    producer_package: PackageIdentityV1,
    finalized: &PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<SealedProtectedWorkerV3HsacoPublicationIntentV1, WorkerV3HsacoPublicationErrorV1> {
    if !finalized
        .finalized_output_identity()
        .matches(finalized.exact_finalized_bytes())
    {
        return Err(WorkerV3HsacoPublicationErrorV1::FinalizedOutputMismatch);
    }
    let raw = derive_unfinalized_hsaco_from_finalized_v1(finalized.exact_finalized_bytes())?;
    if !finalized.raw_output_identity().matches(&raw) {
        return Err(WorkerV3HsacoPublicationErrorV1::RawOutputMismatch);
    }
    let manifest = finalized
        .outer_handoff()
        .module_handoff()
        .symbol_manifest()
        .identity();
    let kernel_set = KernelSetIdentityV1::from_bytes(hash_identity(KERNEL_SET_DOMAIN_V1, |hash| {
        hash.update(manifest.sha256());
        hash.update(manifest.byte_len().to_le_bytes());
        hash.update(finalized.outer_handoff_identity().sha256());
    }));
    let target_text = finalized.target().to_string();
    let target = TargetIdentityV1::from_bytes(hash_identity(TARGET_DOMAIN_V1, |hash| {
        hash_blob(hash, target_text.as_bytes());
        hash.update([finalized.code_object_version().number()]);
        hash.update(finalized.policy_identity().as_bytes());
    }));
    let scope = LinkPublicationScopeV1::new(producer_package, kernel_set, target);
    let expectation = finalized.binding_expectation();
    let request =
        CanonicalLinkRequestIdentityV1::from_bytes(hash_identity(REQUEST_DOMAIN_V1, |hash| {
            hash_attempt(hash, finalized.attempt());
            hash.update([finalized.handoff_slot() as u8]);
            hash.update(finalized.transaction_identity().as_bytes());
            hash.update(finalized.outer_handoff_identity().sha256());
            hash.update(finalized.binding_identity().as_bytes());
            hash.update(finalized.source_evidence_identity().as_bytes());
            hash.update(finalized.raw_inspection_identity().as_bytes());
            hash.update(finalized.link_plan_identity().as_bytes());
            hash.update(finalized.policy_identity().as_bytes());
            hash.update(expectation.invocation_digest());
        }));
    let measurement = finalized.worker_measurement();
    let worker = PinnedWorkerIdentityV1::from_bytes(hash_identity(WORKER_DOMAIN_V1, |hash| {
        hash_content(hash, measurement.executable());
        hash_blob(hash, measurement.worker_build_identity().as_bytes());
        hash_blob(hash, measurement.llvm_build_identity().as_bytes());
    }));
    let response =
        ValidatedResponseIdentityV1::from_bytes(hash_identity(RESPONSE_DOMAIN_V1, |hash| {
            hash.update(finalized.source_evidence_identity().as_bytes());
            hash.update(finalized.raw_inspection_identity().as_bytes());
            hash_content(hash, finalized.raw_output_identity());
        }));
    let linked_output =
        LinkedOutputIdentityV1::from_bytes(*finalized.raw_output_identity().sha256());
    let finalization =
        FinalizationIdentityV1::from_bytes(hash_identity(FINALIZATION_DOMAIN_V1, |hash| {
            hash.update(finalized.identity().as_bytes());
            hash.update(finalized.canonical_digest().as_bytes());
            hash_content(hash, finalized.canonical_descriptor_evidence_identity());
            hash_content(hash, finalized.raw_output_identity());
            hash_content(hash, finalized.finalized_output_identity());
        }));
    let finalized_output =
        FinalizedOutputIdentityV1::from_bytes(*finalized.finalized_output_identity().sha256());
    let publication =
        AtomicPublicationIdentityV1::from_bytes(hash_identity(PUBLICATION_DOMAIN_V1, |hash| {
            hash_attempt(hash, finalized.attempt());
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
        finalized.attempt(),
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
            hash.update(finalized.source_evidence_identity().as_bytes());
            hash.update(finalized.binding_identity().as_bytes());
            hash.update(finalization.as_bytes());
        },
    ));
    Ok(SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        plan,
        upstream,
        finalization: finalized.identity(),
        source: finalized.source_evidence_identity(),
        binding: finalized.binding_identity(),
        raw_inspection: finalized.raw_inspection_identity(),
        raw_output: finalized.raw_output_identity(),
        finalized_output: finalized.finalized_output_identity(),
    })
}

fn try_vec<T>(
    capacity: usize,
    component: &'static str,
) -> Result<Vec<T>, WorkerV3HsacoPublicationErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| WorkerV3HsacoPublicationErrorV1::AllocationFailed { component })?;
    Ok(values)
}

fn try_copy_bytes(
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

fn hash_content(hash: &mut Sha256, identity: ContentIdentityV1) {
    hash.update(identity.sha256());
    hash.update(identity.byte_len().to_le_bytes());
}
