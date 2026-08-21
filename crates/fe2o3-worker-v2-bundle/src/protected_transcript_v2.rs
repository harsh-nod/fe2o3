use std::{fmt, path::Path};

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BackendPublicationReceiptV2, BuildAttempt, BuildInvocation,
    BuildSession, CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1,
    FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1,
    LinkedOutputIdentityV1, MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, PackageIdentityV1,
    PinnedWorkerIdentityV1, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
    ValidatedResponseIdentityV1, WorkerV2PublicationIntentIdentityV2,
    WorkerV2PublicationIntentRecordV2,
};
use fe2o3_artifacts::ArtifactContainerV1;
use fe2o3_build_authority::{CompilerClosureErrorV2, CompilerClosureV2};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, InspectedProtectedRawWorkerV2HsacoV1,
    PreparedFinalizedProtectedWorkerV2HsacoV2,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::DescriptorLineageV1;
use crate::protected_v2::{
    derive_abi_identity_v2, derive_resource_identity_v2, derive_symbol_identity_v2,
    derive_target_identity_v2,
};

const INTENT_TRANSCRIPT_DOMAIN_V2: &[u8] =
    b"FE2O3/PROTECTED-WORKER-V2/PUBLICATION-INTENT-TRANSCRIPT/V2\0";
const INSPECTION_TRANSCRIPT_DOMAIN_V2: &[u8] =
    b"FE2O3/PROTECTED-WORKER-V2/FINALIZER-INSPECTION-TRANSCRIPT/V2\0";
const SOURCE_INTENT_RECORD_MAGIC_V2: &[u8] = b"FE2O3-WORKER-V2-PUBLICATION-INTENT-V2\0";
const SOURCE_INTENT_RECORD_VERSION_V2: u16 = 2;
const SOURCE_INTENT_SLOT_DOMAIN_V2: &[u8] = b"fe2o3.worker-v2-publication-intent.slot.v2\0";
const SOURCE_INTENT_PRODUCER_DOMAIN_V2: &[u8] = b"fe2o3.worker-v2-publication-intent.producer.v2\0";
const RECEIPT_PRODUCER_DOMAIN_V2: &[u8] = b"fe2o3.backend-receipt.producer.v2\0";
const RECEIPT_ATTEMPT_DOMAIN_V2: &[u8] = b"fe2o3.backend-receipt.attempt.v2\0";
const RECEIPT_SCOPE_DOMAIN_V2: &[u8] = b"fe2o3.backend-receipt.scope.v2\0";
const SOURCE_INTENT_RECORD_CHECKSUM_DOMAIN_V2: &[u8] =
    b"fe2o3.worker-v2-publication-intent.record-checksum.v2\0";
const SOURCE_INTENT_RECORD_IDENTITY_DOMAIN_V2: &[u8] =
    b"fe2o3.worker-v2-publication-intent.record-identity.v2\0";
const SOURCE_INTENT_PLAN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.durable-link.complete-plan.v1\0";
const COMPILER_CLOSURE_BYTES_V2: usize = (6 * 32) + 2 + 32;
const ATTEMPT_BYTES_V2: usize = 8 + 16 + 32;
const PLAN_BYTES_V2: usize = ATTEMPT_BYTES_V2 + (3 * 32) + (7 * 32);
const CONTENT_IDENTITY_BYTES_V2: usize = 32 + 8;
const INTENT_TRANSCRIPT_FIXED_BYTES_V2: usize =
    32 + PLAN_BYTES_V2 + 32 + 32 + 32 + 8 + COMPILER_CLOSURE_BYTES_V2;
const INSPECTION_TRANSCRIPT_FIXED_BYTES_V2: usize = 1
    + 32
    + 32
    + 1
    + 32
    + CONTENT_IDENTITY_BYTES_V2
    + CONTENT_IDENTITY_BYTES_V2
    + 32
    + ATTEMPT_BYTES_V2
    + 1
    + 32
    + COMPILER_CLOSURE_BYTES_V2
    + 2
    + 1
    + 32
    + (5 * 32);
const MAX_TARGET_TEXT_BYTES_V2: usize = 256;
const MAX_PRODUCER_SOURCE_BYTES_V2: usize = 4096;
const MAX_PRODUCER_CRATE_NAME_BYTES_V2: usize = 128;

pub(crate) const MAX_PUBLICATION_INTENT_TRANSCRIPT_BYTES_V2: usize =
    INTENT_TRANSCRIPT_FIXED_BYTES_V2
        + 2
        + MAX_PRODUCER_SOURCE_BYTES_V2
        + 2
        + MAX_PRODUCER_CRATE_NAME_BYTES_V2;
pub(crate) const MAX_PROTECTED_INSPECTION_TRANSCRIPT_BYTES_V2: usize =
    INSPECTION_TRANSCRIPT_FIXED_BYTES_V2 + MAX_TARGET_TEXT_BYTES_V2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV2PublicationIntentTranscriptIdentityV2([u8; 32]);

impl WorkerV2PublicationIntentTranscriptIdentityV2 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV2ProtectedInspectionTranscriptIdentityV2([u8; 32]);

impl WorkerV2ProtectedInspectionTranscriptIdentityV2 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact bounded producer-name preimage joining the intent and receipt producer domains.
///
/// This value is inert naming evidence. It does not authenticate the named source or crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2ProducerBindingV2 {
    stable_source: String,
    crate_name: String,
}

impl WorkerV2ProducerBindingV2 {
    pub fn from_codegen(
        crate_name: &str,
        local_source: Option<&Path>,
    ) -> Result<Self, WorkerV2TranscriptValidationErrorV2> {
        let stable_source = match local_source {
            Some(path) => format!(
                "path:{}",
                path.to_str()
                    .ok_or(WorkerV2TranscriptValidationErrorV2::ProducerBinding)?
            ),
            None => format!("crate:{crate_name}"),
        };
        Self::from_stable_source(stable_source, crate_name.to_owned())
    }

    pub fn stable_source(&self) -> &str {
        &self.stable_source
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    fn from_stable_source(
        stable_source: String,
        crate_name: String,
    ) -> Result<Self, WorkerV2TranscriptValidationErrorV2> {
        let value = Self {
            stable_source,
            crate_name,
        };
        value.validate()?;
        Ok(value)
    }

    fn intent_identity(&self) -> [u8; 32] {
        producer_identity(SOURCE_INTENT_PRODUCER_DOMAIN_V2, self)
    }

    pub(crate) fn receipt_identity(&self) -> [u8; 32] {
        producer_identity(RECEIPT_PRODUCER_DOMAIN_V2, self)
    }

    fn validate(&self) -> Result<(), WorkerV2TranscriptValidationErrorV2> {
        if self.stable_source.is_empty()
            || self.stable_source.len() > MAX_PRODUCER_SOURCE_BYTES_V2
            || self.stable_source.ends_with(':')
            || self.stable_source.as_bytes().contains(&0)
            || self.crate_name.is_empty()
            || self.crate_name.len() > MAX_PRODUCER_CRATE_NAME_BYTES_V2
            || !self
                .crate_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(WorkerV2TranscriptValidationErrorV2::ProducerBinding);
        }
        Ok(())
    }
}

/// Complete inert preimage of one persisted protected publication intent.
///
/// Construction requires the artifact-transaction crate's typed V2 record. The source record
/// identity is retained for comparison with durable state; this transcript's own identity is
/// independently recomputable from every public record field after decoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentTranscriptV2 {
    producer_binding: WorkerV2ProducerBindingV2,
    source_record_identity: WorkerV2PublicationIntentIdentityV2,
    plan: DurableLinkPublicationPlanV1,
    producer_identity: [u8; 32],
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    output_identity: FinalizedOutputIdentityV1,
    output_length: u64,
    compiler_closure: CompilerClosureV2,
}

impl WorkerV2PublicationIntentTranscriptV2 {
    pub fn from_record(
        record: WorkerV2PublicationIntentRecordV2,
        producer_binding: WorkerV2ProducerBindingV2,
    ) -> Result<Self, WorkerV2TranscriptValidationErrorV2> {
        let output_length = u64::try_from(record.output_length())
            .map_err(|_| WorkerV2TranscriptValidationErrorV2::OutputLength)?;
        let value = Self {
            producer_binding,
            source_record_identity: record.identity(),
            plan: record.plan(),
            producer_identity: record.producer_identity(),
            upstream_evidence: record.upstream_evidence(),
            output_identity: record.output_identity(),
            output_length,
            compiler_closure: record.compiler_closure(),
        };
        value.validate_self()?;
        if !value.matches_source_record(record) {
            return Err(WorkerV2TranscriptValidationErrorV2::SourceRecord);
        }
        Ok(value)
    }

    pub const fn source_record_identity(&self) -> WorkerV2PublicationIntentIdentityV2 {
        self.source_record_identity
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.plan.attempt()
    }

    pub const fn plan(&self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    pub const fn package_identity(&self) -> PackageIdentityV1 {
        self.plan.scope().package()
    }

    pub const fn producer_identity(&self) -> [u8; 32] {
        self.producer_identity
    }

    pub const fn producer_binding(&self) -> &WorkerV2ProducerBindingV2 {
        &self.producer_binding
    }

    pub const fn upstream_evidence(&self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream_evidence
    }

    pub const fn output_identity(&self) -> FinalizedOutputIdentityV1 {
        self.output_identity
    }

    pub const fn output_length(&self) -> u64 {
        self.output_length
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub fn identity(&self) -> WorkerV2PublicationIntentTranscriptIdentityV2 {
        WorkerV2PublicationIntentTranscriptIdentityV2(hash_domain_bytes(
            INTENT_TRANSCRIPT_DOMAIN_V2,
            &self.canonical_bytes(),
        ))
    }

    pub fn matches_source_record(&self, record: WorkerV2PublicationIntentRecordV2) -> bool {
        self.source_record_identity == record.identity()
            && self.plan.attempt() == record.attempt()
            && self.plan == record.plan()
            && self.producer_identity == record.producer_identity()
            && self.upstream_evidence == record.upstream_evidence()
            && self.output_identity == record.output_identity()
            && usize::try_from(self.output_length) == Ok(record.output_length())
            && self.compiler_closure == record.compiler_closure()
    }

    pub fn matches_backend_receipt(&self, receipt: BackendPublicationReceiptV2) -> bool {
        let attempt = self.plan.attempt();
        let scope = self.plan.scope();
        let attempt_identity = sha256_concat(&[
            RECEIPT_ATTEMPT_DOMAIN_V2,
            &attempt.generation().to_le_bytes(),
            attempt.session().as_bytes(),
            attempt.invocation().as_bytes(),
        ]);
        let scope_identity = sha256_concat(&[
            RECEIPT_SCOPE_DOMAIN_V2,
            scope.package().as_bytes(),
            scope.kernel_set().as_bytes(),
            scope.target().as_bytes(),
        ]);
        receipt.attempt_identity() == attempt_identity
            && receipt.producer_identity() == self.producer_binding.receipt_identity()
            && receipt.scope_identity() == scope_identity
            && receipt.plan_commitment() == derive_plan_identity(self.plan)
            && receipt.upstream_evidence_identity() == self.upstream_evidence.as_bytes()
            && receipt.finalized_output_identity() == *self.output_identity.as_bytes()
            && receipt.publication_identity() == *self.plan.publication().as_bytes()
            && receipt.compiler_closure() == self.compiler_closure
    }

    pub const fn grants_compiler_authority(&self) -> bool {
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

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            INTENT_TRANSCRIPT_FIXED_BYTES_V2
                + 4
                + self.producer_binding.stable_source.len()
                + self.producer_binding.crate_name.len(),
        );
        encode_text(&mut bytes, &self.producer_binding.stable_source);
        encode_text(&mut bytes, &self.producer_binding.crate_name);
        bytes.extend_from_slice(&self.source_record_identity.as_bytes());
        encode_plan(&mut bytes, self.plan);
        bytes.extend_from_slice(&self.producer_identity);
        bytes.extend_from_slice(&self.upstream_evidence.as_bytes());
        bytes.extend_from_slice(self.output_identity.as_bytes());
        bytes.extend_from_slice(&self.output_length.to_le_bytes());
        encode_compiler_closure(&mut bytes, self.compiler_closure);
        bytes
    }

    pub(crate) fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV2TranscriptDecodeErrorV2> {
        if bytes.len() > MAX_PUBLICATION_INTENT_TRANSCRIPT_BYTES_V2
            || bytes.len() < INTENT_TRANSCRIPT_FIXED_BYTES_V2 + 4 + 2
        {
            return Err(WorkerV2TranscriptDecodeErrorV2::Length);
        }
        let mut reader = TranscriptReaderV2::new(bytes);
        let producer_binding = WorkerV2ProducerBindingV2::from_stable_source(
            decode_text(&mut reader, MAX_PRODUCER_SOURCE_BYTES_V2)?,
            decode_text(&mut reader, MAX_PRODUCER_CRATE_NAME_BYTES_V2)?,
        )
        .map_err(WorkerV2TranscriptDecodeErrorV2::Validation)?;
        let value = Self {
            producer_binding,
            source_record_identity: WorkerV2PublicationIntentIdentityV2::from_bytes(
                reader.array()?,
            ),
            plan: decode_plan(&mut reader)?,
            producer_identity: reader.array()?,
            upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1::from_bytes(reader.array()?),
            output_identity: FinalizedOutputIdentityV1::from_bytes(reader.array()?),
            output_length: reader.u64()?,
            compiler_closure: decode_compiler_closure(&mut reader)?,
        };
        if !reader.finished() {
            return Err(WorkerV2TranscriptDecodeErrorV2::TrailingBytes);
        }
        value
            .validate_self()
            .map_err(WorkerV2TranscriptDecodeErrorV2::Validation)?;
        if value.canonical_bytes() != bytes {
            return Err(WorkerV2TranscriptDecodeErrorV2::NonCanonical);
        }
        Ok(value)
    }

    fn validate_self(&self) -> Result<(), WorkerV2TranscriptValidationErrorV2> {
        self.producer_binding.validate()?;
        if self.source_record_identity.as_bytes() == [0; 32] || self.producer_identity == [0; 32] {
            return Err(WorkerV2TranscriptValidationErrorV2::ZeroIdentity);
        }
        if self.plan.attempt().generation() == 0 {
            return Err(WorkerV2TranscriptValidationErrorV2::Attempt);
        }
        if self.output_identity != self.plan.finalized_output()
            || self.output_length == 0
            || self.output_length > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES as u64
            || usize::try_from(self.output_length).is_err()
        {
            return Err(WorkerV2TranscriptValidationErrorV2::OutputLength);
        }
        if self.producer_identity != self.producer_binding.intent_identity() {
            return Err(WorkerV2TranscriptValidationErrorV2::ProducerBinding);
        }
        if self.source_record_identity.as_bytes() != derive_source_intent_identity(self) {
            return Err(WorkerV2TranscriptValidationErrorV2::SourceRecord);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2ProtectedInspectionRouteV2 {
    InspectedRaw,
    CanonicallyFinalized,
}

/// Canonical inert transcript of typed finalizer inspection/finalization evidence.
///
/// The transcript retains full recomputable bindings rather than treating the raw inspection or
/// finalization digest as self-authenticating. Runtime admission can compare it back to the typed
/// finalizer source with `matches_*_source` before granting any authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2ProtectedInspectionTranscriptV2 {
    route: WorkerV2ProtectedInspectionRouteV2,
    source_evidence_identity: [u8; 32],
    raw_inspection_identity: [u8; 32],
    canonical_finalization_identity: Option<[u8; 32]>,
    raw_bytes: ContentIdentityV1,
    final_bytes: ContentIdentityV1,
    policy_identity: [u8; 32],
    attempt: BuildAttempt,
    handoff_slot: u8,
    handoff_identity: [u8; 32],
    compiler_closure: CompilerClosureV2,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    canonical_code_object_digest: [u8; 32],
    target_identity: [u8; 32],
    abi_identity: [u8; 32],
    descriptor_identity: [u8; 32],
    symbol_identity: [u8; 32],
    resource_identity: [u8; 32],
}

impl WorkerV2ProtectedInspectionTranscriptV2 {
    pub fn from_inspected(
        source: &InspectedProtectedRawWorkerV2HsacoV1,
        container: &ArtifactContainerV1,
        descriptor_lineage: &DescriptorLineageV1,
    ) -> Result<Self, WorkerV2TranscriptValidationErrorV2> {
        let raw_bytes = ContentIdentityV1::calculate(source.exact_bytes());
        let value = Self {
            route: WorkerV2ProtectedInspectionRouteV2::InspectedRaw,
            source_evidence_identity: *source.source_evidence_identity().as_bytes(),
            raw_inspection_identity: *source.identity().as_bytes(),
            canonical_finalization_identity: None,
            raw_bytes,
            final_bytes: raw_bytes,
            policy_identity: *source.policy().identity().as_bytes(),
            attempt: source.attempt(),
            handoff_slot: source.handoff_slot() as u8,
            handoff_identity: *source.handoff_identity().as_bytes(),
            compiler_closure: source.compiler_closure(),
            target: source.target(),
            code_object_version: source.code_object_version(),
            canonical_code_object_digest: *descriptor_lineage
                .table()
                .canonical_code_object_digest()
                .as_bytes(),
            target_identity: derive_target_identity_v2(container),
            abi_identity: derive_abi_identity_v2(container),
            descriptor_identity: descriptor_identity(descriptor_lineage),
            symbol_identity: derive_symbol_identity_v2(container),
            resource_identity: derive_resource_identity_v2(container),
        };
        value.validate_source_facets(container, descriptor_lineage)?;
        value.validate_self()?;
        Ok(value)
    }

    pub fn from_finalized(
        source: &PreparedFinalizedProtectedWorkerV2HsacoV2,
        container: &ArtifactContainerV1,
        descriptor_lineage: &DescriptorLineageV1,
    ) -> Result<Self, WorkerV2TranscriptValidationErrorV2> {
        let value = Self {
            route: WorkerV2ProtectedInspectionRouteV2::CanonicallyFinalized,
            source_evidence_identity: *source.source_evidence_identity().as_bytes(),
            raw_inspection_identity: *source.raw_inspection_identity().as_bytes(),
            canonical_finalization_identity: Some(*source.identity().as_bytes()),
            raw_bytes: source.raw_output_identity(),
            final_bytes: source.finalized_output_identity(),
            policy_identity: *source.policy_identity().as_bytes(),
            attempt: source.attempt(),
            handoff_slot: source.handoff_slot() as u8,
            handoff_identity: *source.handoff_identity().as_bytes(),
            compiler_closure: source.compiler_closure(),
            target: source.target(),
            code_object_version: source.code_object_version(),
            canonical_code_object_digest: *source.canonical_digest().as_bytes(),
            target_identity: derive_target_identity_v2(container),
            abi_identity: derive_abi_identity_v2(container),
            descriptor_identity: descriptor_identity(descriptor_lineage),
            symbol_identity: derive_symbol_identity_v2(container),
            resource_identity: derive_resource_identity_v2(container),
        };
        value.validate_source_facets(container, descriptor_lineage)?;
        value.validate_self()?;
        Ok(value)
    }

    pub const fn route(&self) -> WorkerV2ProtectedInspectionRouteV2 {
        self.route
    }

    pub const fn source_evidence_identity(&self) -> [u8; 32] {
        self.source_evidence_identity
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn raw_bytes_identity(&self) -> ContentIdentityV1 {
        self.raw_bytes
    }

    pub const fn final_bytes_identity(&self) -> ContentIdentityV1 {
        self.final_bytes
    }

    pub const fn raw_inspection_identity(&self) -> [u8; 32] {
        self.raw_inspection_identity
    }

    pub const fn canonical_finalization_identity(&self) -> Option<[u8; 32]> {
        self.canonical_finalization_identity
    }

    pub const fn canonical_code_object_digest(&self) -> [u8; 32] {
        self.canonical_code_object_digest
    }

    pub const fn policy_identity(&self) -> [u8; 32] {
        self.policy_identity
    }

    pub const fn handoff_slot(&self) -> u8 {
        self.handoff_slot
    }

    pub const fn handoff_identity(&self) -> [u8; 32] {
        self.handoff_identity
    }

    pub const fn target_identity(&self) -> [u8; 32] {
        self.target_identity
    }

    pub const fn abi_identity(&self) -> [u8; 32] {
        self.abi_identity
    }

    pub const fn descriptor_identity(&self) -> [u8; 32] {
        self.descriptor_identity
    }

    pub const fn symbol_identity(&self) -> [u8; 32] {
        self.symbol_identity
    }

    pub const fn resource_identity(&self) -> [u8; 32] {
        self.resource_identity
    }

    pub fn identity(&self) -> WorkerV2ProtectedInspectionTranscriptIdentityV2 {
        WorkerV2ProtectedInspectionTranscriptIdentityV2(hash_domain_bytes(
            INSPECTION_TRANSCRIPT_DOMAIN_V2,
            &self.canonical_bytes(),
        ))
    }

    pub fn matches_inspected_source(&self, source: &InspectedProtectedRawWorkerV2HsacoV1) -> bool {
        self.route == WorkerV2ProtectedInspectionRouteV2::InspectedRaw
            && self.source_evidence_identity == *source.source_evidence_identity().as_bytes()
            && self.raw_inspection_identity == *source.identity().as_bytes()
            && self.canonical_finalization_identity.is_none()
            && self.raw_bytes.matches(source.exact_bytes())
            && self.final_bytes == self.raw_bytes
            && self.policy_identity == *source.policy().identity().as_bytes()
            && self.attempt == source.attempt()
            && self.handoff_slot == source.handoff_slot() as u8
            && self.handoff_identity == *source.handoff_identity().as_bytes()
            && self.compiler_closure == source.compiler_closure()
            && self.target == source.target()
            && self.code_object_version == source.code_object_version()
    }

    pub fn matches_finalized_source(
        &self,
        source: &PreparedFinalizedProtectedWorkerV2HsacoV2,
    ) -> bool {
        self.route == WorkerV2ProtectedInspectionRouteV2::CanonicallyFinalized
            && self.source_evidence_identity == *source.source_evidence_identity().as_bytes()
            && self.raw_inspection_identity == *source.raw_inspection_identity().as_bytes()
            && self.canonical_finalization_identity == Some(*source.identity().as_bytes())
            && self.raw_bytes == source.raw_output_identity()
            && self.final_bytes == source.finalized_output_identity()
            && self.final_bytes.matches(source.exact_finalized_bytes())
            && self.policy_identity == *source.policy_identity().as_bytes()
            && self.attempt == source.attempt()
            && self.handoff_slot == source.handoff_slot() as u8
            && self.handoff_identity == *source.handoff_identity().as_bytes()
            && self.compiler_closure == source.compiler_closure()
            && self.target == source.target()
            && self.code_object_version == source.code_object_version()
            && self.canonical_code_object_digest == *source.canonical_digest().as_bytes()
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

    pub(crate) fn validates_exact_raw_bytes(&self, bytes: &[u8]) -> bool {
        self.raw_bytes.matches(bytes)
    }

    pub(crate) fn validates_exact_final_bytes(&self, bytes: &[u8]) -> bool {
        self.final_bytes.matches(bytes)
    }

    pub(crate) fn validate_facets(
        &self,
        container: &ArtifactContainerV1,
        descriptor_lineage: &DescriptorLineageV1,
    ) -> Result<(), WorkerV2TranscriptValidationErrorV2> {
        self.validate_source_facets(container, descriptor_lineage)
    }

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let target = self.target.to_string();
        let mut bytes = Vec::with_capacity(INSPECTION_TRANSCRIPT_FIXED_BYTES_V2 + target.len());
        bytes.push(match self.route {
            WorkerV2ProtectedInspectionRouteV2::InspectedRaw => 0,
            WorkerV2ProtectedInspectionRouteV2::CanonicallyFinalized => 1,
        });
        bytes.extend_from_slice(&self.source_evidence_identity);
        bytes.extend_from_slice(&self.raw_inspection_identity);
        bytes.push(u8::from(self.canonical_finalization_identity.is_some()));
        bytes.extend_from_slice(&self.canonical_finalization_identity.unwrap_or([0; 32]));
        encode_content_identity(&mut bytes, self.raw_bytes);
        encode_content_identity(&mut bytes, self.final_bytes);
        bytes.extend_from_slice(&self.policy_identity);
        encode_attempt(&mut bytes, self.attempt);
        bytes.push(self.handoff_slot);
        bytes.extend_from_slice(&self.handoff_identity);
        encode_compiler_closure(&mut bytes, self.compiler_closure);
        bytes.extend_from_slice(&(target.len() as u16).to_le_bytes());
        bytes.extend_from_slice(target.as_bytes());
        bytes.push(code_object_version_tag(self.code_object_version));
        bytes.extend_from_slice(&self.canonical_code_object_digest);
        for identity in [
            self.target_identity,
            self.abi_identity,
            self.descriptor_identity,
            self.symbol_identity,
            self.resource_identity,
        ] {
            bytes.extend_from_slice(&identity);
        }
        bytes
    }

    pub(crate) fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV2TranscriptDecodeErrorV2> {
        if bytes.len() > MAX_PROTECTED_INSPECTION_TRANSCRIPT_BYTES_V2
            || bytes.len() < INSPECTION_TRANSCRIPT_FIXED_BYTES_V2
        {
            return Err(WorkerV2TranscriptDecodeErrorV2::Length);
        }
        let mut reader = TranscriptReaderV2::new(bytes);
        let route = match reader.u8()? {
            0 => WorkerV2ProtectedInspectionRouteV2::InspectedRaw,
            1 => WorkerV2ProtectedInspectionRouteV2::CanonicallyFinalized,
            _ => return Err(WorkerV2TranscriptDecodeErrorV2::Tag),
        };
        let source_evidence_identity = reader.array()?;
        let raw_inspection_identity = reader.array()?;
        let finalization_present = reader.u8()?;
        if finalization_present > 1 {
            return Err(WorkerV2TranscriptDecodeErrorV2::Tag);
        }
        let finalization_bytes = reader.array()?;
        let canonical_finalization_identity = match finalization_present {
            0 if finalization_bytes == [0; 32] => None,
            1 if finalization_bytes != [0; 32] => Some(finalization_bytes),
            _ => return Err(WorkerV2TranscriptDecodeErrorV2::NonCanonical),
        };
        let raw_bytes = decode_content_identity(&mut reader)?;
        let final_bytes = decode_content_identity(&mut reader)?;
        let policy_identity = reader.array()?;
        let attempt = decode_attempt(&mut reader)?;
        let handoff_slot = reader.u8()?;
        if handoff_slot > 2 {
            return Err(WorkerV2TranscriptDecodeErrorV2::Tag);
        }
        let handoff_identity = reader.array()?;
        let compiler_closure = decode_compiler_closure(&mut reader)?;
        let target_len = usize::from(reader.u16()?);
        if target_len == 0 || target_len > MAX_TARGET_TEXT_BYTES_V2 {
            return Err(WorkerV2TranscriptDecodeErrorV2::Length);
        }
        let target_text = std::str::from_utf8(reader.take(target_len)?)
            .map_err(|_| WorkerV2TranscriptDecodeErrorV2::Target)?;
        let target = DeviceTargetV1::parse(target_text)
            .map_err(|_| WorkerV2TranscriptDecodeErrorV2::Target)?;
        let code_object_version =
            decode_code_object_version(reader.u8()?).ok_or(WorkerV2TranscriptDecodeErrorV2::Tag)?;
        let value = Self {
            route,
            source_evidence_identity,
            raw_inspection_identity,
            canonical_finalization_identity,
            raw_bytes,
            final_bytes,
            policy_identity,
            attempt,
            handoff_slot,
            handoff_identity,
            compiler_closure,
            target,
            code_object_version,
            canonical_code_object_digest: reader.array()?,
            target_identity: reader.array()?,
            abi_identity: reader.array()?,
            descriptor_identity: reader.array()?,
            symbol_identity: reader.array()?,
            resource_identity: reader.array()?,
        };
        if !reader.finished() {
            return Err(WorkerV2TranscriptDecodeErrorV2::TrailingBytes);
        }
        value
            .validate_self()
            .map_err(WorkerV2TranscriptDecodeErrorV2::Validation)?;
        if value.canonical_bytes() != bytes {
            return Err(WorkerV2TranscriptDecodeErrorV2::NonCanonical);
        }
        Ok(value)
    }

    fn validate_source_facets(
        &self,
        container: &ArtifactContainerV1,
        descriptor_lineage: &DescriptorLineageV1,
    ) -> Result<(), WorkerV2TranscriptValidationErrorV2> {
        let table = descriptor_lineage.table();
        if self.target != table.device_target()
            || self.code_object_version != table.code_object_version()
            || self.target_identity != derive_target_identity_v2(container)
            || self.abi_identity != derive_abi_identity_v2(container)
            || self.descriptor_identity != descriptor_identity(descriptor_lineage)
            || self.symbol_identity != derive_symbol_identity_v2(container)
            || self.resource_identity != derive_resource_identity_v2(container)
        {
            return Err(WorkerV2TranscriptValidationErrorV2::Facet);
        }
        if self.canonical_code_object_digest != *table.canonical_code_object_digest().as_bytes() {
            return Err(WorkerV2TranscriptValidationErrorV2::CanonicalDigest);
        }
        Ok(())
    }

    fn validate_self(&self) -> Result<(), WorkerV2TranscriptValidationErrorV2> {
        if self.source_evidence_identity == [0; 32]
            || self.raw_inspection_identity == [0; 32]
            || self.policy_identity == [0; 32]
            || self.handoff_identity == [0; 32]
            || [
                self.target_identity,
                self.abi_identity,
                self.descriptor_identity,
                self.symbol_identity,
                self.resource_identity,
            ]
            .contains(&[0; 32])
        {
            return Err(WorkerV2TranscriptValidationErrorV2::ZeroIdentity);
        }
        if self.raw_bytes.byte_len() == 0
            || self.final_bytes.byte_len() == 0
            || self.raw_bytes.byte_len() > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES as u64
            || self.final_bytes.byte_len() > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES as u64
        {
            return Err(WorkerV2TranscriptValidationErrorV2::OutputLength);
        }
        match self.route {
            WorkerV2ProtectedInspectionRouteV2::InspectedRaw => {
                if self.canonical_finalization_identity.is_some()
                    || self.raw_bytes != self.final_bytes
                {
                    return Err(WorkerV2TranscriptValidationErrorV2::Route);
                }
            }
            WorkerV2ProtectedInspectionRouteV2::CanonicallyFinalized => {
                if self.canonical_finalization_identity.is_none()
                    || self.canonical_code_object_digest == [0; 32]
                {
                    return Err(WorkerV2TranscriptValidationErrorV2::Route);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV2TranscriptValidationErrorV2 {
    ZeroIdentity,
    Attempt,
    OutputLength,
    SourceRecord,
    ProducerBinding,
    Route,
    Facet,
    CanonicalDigest,
}

impl fmt::Display for WorkerV2TranscriptValidationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ZeroIdentity => "protected transcript contains a zero identity",
            Self::Attempt => "protected transcript build attempt is invalid",
            Self::OutputLength => "protected transcript output identity or length is invalid",
            Self::SourceRecord => "protected transcript differs from its typed source record",
            Self::ProducerBinding => {
                "protected transcript producer binding is invalid or incoherent"
            }
            Self::Route => "protected inspection route and finalization lineage disagree",
            Self::Facet => "protected inspection transcript facet does not match the bundle",
            Self::CanonicalDigest => {
                "protected finalization digest does not match the descriptor table"
            }
        })
    }
}

impl std::error::Error for WorkerV2TranscriptValidationErrorV2 {}

#[derive(Debug)]
pub(crate) enum WorkerV2TranscriptDecodeErrorV2 {
    Length,
    Truncated,
    TrailingBytes,
    Tag,
    Target,
    Attempt,
    CompilerClosure(CompilerClosureErrorV2),
    Validation(WorkerV2TranscriptValidationErrorV2),
    NonCanonical,
}

impl fmt::Display for WorkerV2TranscriptDecodeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => formatter.write_str("protected transcript length is invalid"),
            Self::Truncated => formatter.write_str("protected transcript is truncated"),
            Self::TrailingBytes => formatter.write_str("protected transcript has trailing bytes"),
            Self::Tag => formatter.write_str("protected transcript contains an unknown tag"),
            Self::Target => formatter.write_str("protected transcript target is invalid"),
            Self::Attempt => formatter.write_str("protected transcript attempt is invalid"),
            Self::CompilerClosure(error) => write!(
                formatter,
                "protected transcript compiler closure is invalid: {error}"
            ),
            Self::Validation(error) => write!(
                formatter,
                "protected transcript coherence validation failed: {error}"
            ),
            Self::NonCanonical => formatter.write_str("protected transcript is not canonical"),
        }
    }
}

fn derive_source_intent_identity(transcript: &WorkerV2PublicationIntentTranscriptV2) -> [u8; 32] {
    let plan = transcript.plan;
    let attempt = plan.attempt();
    let slot = sha256_concat(&[
        SOURCE_INTENT_SLOT_DOMAIN_V2,
        &transcript.producer_identity,
        &attempt.generation().to_le_bytes(),
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
    ]);
    let mut body = Vec::new();
    body.extend_from_slice(SOURCE_INTENT_RECORD_MAGIC_V2);
    body.extend_from_slice(&SOURCE_INTENT_RECORD_VERSION_V2.to_le_bytes());
    body.extend_from_slice(&slot);
    encode_attempt(&mut body, attempt);
    body.extend_from_slice(&transcript.producer_identity);
    body.extend_from_slice(&transcript.upstream_evidence.as_bytes());
    body.extend_from_slice(&derive_plan_identity(plan));
    encode_plan_fields(&mut body, plan);
    body.extend_from_slice(transcript.output_identity.as_bytes());
    body.extend_from_slice(&transcript.output_length.to_le_bytes());
    encode_compiler_closure(&mut body, transcript.compiler_closure);
    let checksum = sha256_concat(&[SOURCE_INTENT_RECORD_CHECKSUM_DOMAIN_V2, &body]);
    body.extend_from_slice(&checksum);
    sha256_concat(&[SOURCE_INTENT_RECORD_IDENTITY_DOMAIN_V2, &body])
}

fn derive_plan_identity(plan: DurableLinkPublicationPlanV1) -> [u8; 32] {
    let attempt = plan.attempt();
    let scope = plan.scope();
    sha256_concat(&[
        SOURCE_INTENT_PLAN_IDENTITY_DOMAIN_V1,
        &attempt.generation().to_le_bytes(),
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
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
    ])
}

fn sha256_concat(parts: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part);
    }
    digest.finalize().into()
}

fn producer_identity(domain: &[u8], binding: &WorkerV2ProducerBindingV2) -> [u8; 32] {
    sha256_concat(&[
        domain,
        &(binding.stable_source.len() as u64).to_le_bytes(),
        binding.stable_source.as_bytes(),
        &(binding.crate_name.len() as u64).to_le_bytes(),
        binding.crate_name.as_bytes(),
    ])
}

fn descriptor_identity(descriptor_lineage: &DescriptorLineageV1) -> [u8; 32] {
    hash_domain_bytes(
        b"FE2O3/PROTECTED-WORKER-V2/DESCRIPTOR-IDENTITY/V2\0",
        &descriptor_lineage.canonical_bytes(),
    )
}

fn hash_domain_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn encode_attempt(bytes: &mut Vec<u8>, attempt: BuildAttempt) {
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(attempt.session().as_bytes());
    bytes.extend_from_slice(attempt.invocation().as_bytes());
}

fn encode_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn decode_text(
    reader: &mut TranscriptReaderV2<'_>,
    max: usize,
) -> Result<String, WorkerV2TranscriptDecodeErrorV2> {
    let len = usize::from(reader.u16()?);
    if len == 0 || len > max {
        return Err(WorkerV2TranscriptDecodeErrorV2::Length);
    }
    std::str::from_utf8(reader.take(len)?)
        .map(str::to_owned)
        .map_err(|_| WorkerV2TranscriptDecodeErrorV2::Tag)
}

fn decode_attempt(
    reader: &mut TranscriptReaderV2<'_>,
) -> Result<BuildAttempt, WorkerV2TranscriptDecodeErrorV2> {
    let generation = reader.u64()?;
    let session = BuildSession::from_bytes(reader.array()?);
    let invocation = BuildInvocation::from_bytes(reader.array()?);
    BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        session.to_hex(),
        invocation.to_hex()
    ))
    .map_err(|_| WorkerV2TranscriptDecodeErrorV2::Attempt)
}

fn encode_plan(bytes: &mut Vec<u8>, plan: DurableLinkPublicationPlanV1) {
    encode_attempt(bytes, plan.attempt());
    encode_plan_fields(bytes, plan);
}

fn encode_plan_fields(bytes: &mut Vec<u8>, plan: DurableLinkPublicationPlanV1) {
    for identity in [
        *plan.scope().package().as_bytes(),
        *plan.scope().kernel_set().as_bytes(),
        *plan.scope().target().as_bytes(),
        *plan.request().as_bytes(),
        *plan.worker().as_bytes(),
        *plan.response().as_bytes(),
        *plan.linked_output().as_bytes(),
        *plan.finalization().as_bytes(),
        *plan.finalized_output().as_bytes(),
        *plan.publication().as_bytes(),
    ] {
        bytes.extend_from_slice(&identity);
    }
}

fn decode_plan(
    reader: &mut TranscriptReaderV2<'_>,
) -> Result<DurableLinkPublicationPlanV1, WorkerV2TranscriptDecodeErrorV2> {
    let attempt = decode_attempt(reader)?;
    let scope = LinkPublicationScopeV1::new(
        PackageIdentityV1::from_bytes(reader.array()?),
        KernelSetIdentityV1::from_bytes(reader.array()?),
        TargetIdentityV1::from_bytes(reader.array()?),
    );
    Ok(DurableLinkPublicationPlanV1::new(
        attempt,
        scope,
        CanonicalLinkRequestIdentityV1::from_bytes(reader.array()?),
        PinnedWorkerIdentityV1::from_bytes(reader.array()?),
        ValidatedResponseIdentityV1::from_bytes(reader.array()?),
        LinkedOutputIdentityV1::from_bytes(reader.array()?),
        FinalizationIdentityV1::from_bytes(reader.array()?),
        FinalizedOutputIdentityV1::from_bytes(reader.array()?),
        AtomicPublicationIdentityV1::from_bytes(reader.array()?),
    ))
}

fn encode_content_identity(bytes: &mut Vec<u8>, identity: ContentIdentityV1) {
    bytes.extend_from_slice(identity.sha256());
    bytes.extend_from_slice(&identity.byte_len().to_le_bytes());
}

fn decode_content_identity(
    reader: &mut TranscriptReaderV2<'_>,
) -> Result<ContentIdentityV1, WorkerV2TranscriptDecodeErrorV2> {
    Ok(ContentIdentityV1::from_parts(
        reader.array()?,
        reader.u64()?,
    ))
}

fn encode_compiler_closure(bytes: &mut Vec<u8>, closure: CompilerClosureV2) {
    for pin in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        bytes.extend_from_slice(&pin);
    }
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&closure.identity_sha256());
}

fn decode_compiler_closure(
    reader: &mut TranscriptReaderV2<'_>,
) -> Result<CompilerClosureV2, WorkerV2TranscriptDecodeErrorV2> {
    CompilerClosureV2::from_pins_and_identity(
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.u16()?,
        reader.array()?,
    )
    .map_err(WorkerV2TranscriptDecodeErrorV2::CompilerClosure)
}

const fn code_object_version_tag(value: CodeObjectVersion) -> u8 {
    match value {
        CodeObjectVersion::V4 => 0,
        CodeObjectVersion::V5 => 1,
        CodeObjectVersion::V6 => 2,
    }
}

const fn decode_code_object_version(tag: u8) -> Option<CodeObjectVersion> {
    match tag {
        0 => Some(CodeObjectVersion::V4),
        1 => Some(CodeObjectVersion::V5),
        2 => Some(CodeObjectVersion::V6),
        _ => None,
    }
}

struct TranscriptReaderV2<'a> {
    remaining: &'a [u8],
}

impl<'a> TranscriptReaderV2<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn finished(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], WorkerV2TranscriptDecodeErrorV2> {
        if self.remaining.len() < count {
            return Err(WorkerV2TranscriptDecodeErrorV2::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WorkerV2TranscriptDecodeErrorV2> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, WorkerV2TranscriptDecodeErrorV2> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, WorkerV2TranscriptDecodeErrorV2> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV2TranscriptDecodeErrorV2> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closure(seed: u8) -> CompilerClosureV2 {
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

    fn inspection_transcript() -> WorkerV2ProtectedInspectionTranscriptV2 {
        let exact_bytes = [0x41; 64];
        let content = ContentIdentityV1::calculate(&exact_bytes);
        WorkerV2ProtectedInspectionTranscriptV2 {
            route: WorkerV2ProtectedInspectionRouteV2::InspectedRaw,
            source_evidence_identity: [0x11; 32],
            raw_inspection_identity: [0x12; 32],
            canonical_finalization_identity: None,
            raw_bytes: content,
            final_bytes: content,
            policy_identity: [0x13; 32],
            attempt: BuildAttempt::from_env_value(&format!(
                "1:{}:{}",
                BuildSession::from_bytes([0x14; 16]).to_hex(),
                BuildInvocation::from_bytes([0x15; 32]).to_hex()
            ))
            .unwrap(),
            handoff_slot: 1,
            handoff_identity: [0x16; 32],
            compiler_closure: closure(0x21),
            target: DeviceTargetV1::parse("gfx942").unwrap(),
            code_object_version: CodeObjectVersion::V6,
            canonical_code_object_digest: [0; 32],
            target_identity: [0x31; 32],
            abi_identity: [0x32; 32],
            descriptor_identity: [0x33; 32],
            symbol_identity: [0x34; 32],
            resource_identity: [0x35; 32],
        }
    }

    #[test]
    fn inspection_transcript_is_canonical_bounded_and_inert() {
        let transcript = inspection_transcript();
        transcript.validate_self().unwrap();
        let bytes = transcript.canonical_bytes();
        let decoded = WorkerV2ProtectedInspectionTranscriptV2::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.canonical_bytes(), bytes);
        assert_eq!(decoded.identity(), transcript.identity());
        assert!(!decoded.grants_compiler_authority());
        assert!(!decoded.grants_proof_authority());
        assert!(!decoded.grants_load_authority());
        assert!(!decoded.grants_launch_authority());

        let mut truncated = bytes.clone();
        truncated.pop();
        assert!(WorkerV2ProtectedInspectionTranscriptV2::decode_canonical(&truncated).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            WorkerV2ProtectedInspectionTranscriptV2::decode_canonical(&trailing),
            Err(WorkerV2TranscriptDecodeErrorV2::TrailingBytes)
        ));
        assert!(matches!(
            WorkerV2ProtectedInspectionTranscriptV2::decode_canonical(&vec![
                0;
                MAX_PROTECTED_INSPECTION_TRANSCRIPT_BYTES_V2
                    + 1
            ]),
            Err(WorkerV2TranscriptDecodeErrorV2::Length)
        ));
    }

    #[test]
    fn recomputed_inspection_substitution_fails_exact_byte_revalidation() {
        let transcript = inspection_transcript();
        let original_identity = transcript.identity();
        let mut bytes = transcript.canonical_bytes();
        const RAW_CONTENT_OFFSET: usize = 1 + 32 + 32 + 1 + 32;
        const FINAL_CONTENT_OFFSET: usize = RAW_CONTENT_OFFSET + CONTENT_IDENTITY_BYTES_V2;
        bytes[RAW_CONTENT_OFFSET..RAW_CONTENT_OFFSET + 32].fill(0xa1);
        bytes[FINAL_CONTENT_OFFSET..FINAL_CONTENT_OFFSET + 32].fill(0xa1);

        let substituted =
            WorkerV2ProtectedInspectionTranscriptV2::decode_canonical(&bytes).unwrap();
        assert_ne!(substituted.identity(), original_identity);
        assert!(!substituted.validates_exact_raw_bytes(&[0x41; 64]));
        assert!(!substituted.validates_exact_final_bytes(&[0x41; 64]));
    }
}
