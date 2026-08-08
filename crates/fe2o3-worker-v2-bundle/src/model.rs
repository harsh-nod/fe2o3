use fe2o3_artifact_transaction::{
    DurablePublishedHsacoClaimV1, MAX_DURABLE_FINALIZED_ARTIFACT_BYTES,
};
use fe2o3_artifacts::{
    ArtifactContainerV1, BundleIndexV1, CallerClaimedPackageIdentityV1, CodeObjectFormat,
    DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestAlgorithm, DirectLinkBindingSourceV1,
    DirectLinkBundleEvidenceV1, Endianness, MAX_BUNDLE_INDEX_BYTES, MAX_CONTAINER_BYTES,
    MAX_DIRECT_LINK_EVIDENCE_BYTES, ManifestClaimDerivedLinkPublicationScopeV1,
    ManifestClaimDirectLinkPublicationBridgeV1, PayloadDigest, PointerWidth, ProofRecordV1,
};
use fe2o3_kernel_descriptor::{
    DeviceDescriptorTableV1, MAX_DESCRIPTOR_TABLE_BYTES, encode_device_descriptor_table_v1,
};

use crate::{EnvelopeValidationError, PublicationClaimFieldV1};

pub const MAX_WORKER_V2_RAW_HSACO_BYTES: usize = MAX_DURABLE_FINALIZED_ARTIFACT_BYTES;
pub const MAX_WORKER_V2_PROOF_EVIDENCE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_WORKER_V2_LOAD_ENVELOPE_BYTES: usize = MAX_CONTAINER_BYTES
    + MAX_BUNDLE_INDEX_BYTES
    + MAX_DIRECT_LINK_EVIDENCE_BYTES
    + MAX_DESCRIPTOR_TABLE_BYTES
    + MAX_WORKER_V2_RAW_HSACO_BYTES
    + MAX_WORKER_V2_PROOF_EVIDENCE_BYTES
    + 4096;

/// Exact raw linked HSACO bytes and their SHA-256 content identity.
///
/// This is immutable input evidence, not a loadable module or a claim that
/// finalization, inspection, or publication occurred.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactRawHsacoV1 {
    identity: PayloadDigest,
    bytes: Vec<u8>,
}

impl ExactRawHsacoV1 {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, EnvelopeValidationError> {
        Self::new(DigestAlgorithm::Sha256.calculate(&bytes), bytes)
    }

    pub fn new(identity: PayloadDigest, bytes: Vec<u8>) -> Result<Self, EnvelopeValidationError> {
        validate_raw_hsaco(identity, &bytes)?;
        Ok(Self { identity, bytes })
    }

    pub const fn identity(&self) -> PayloadDigest {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact canonical descriptor table retained as inert compiler-lineage data.
///
/// Recovered admission must still inspect the finalized HSACO and verify that
/// this exact table is embedded in those bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescriptorLineageV1 {
    table: DeviceDescriptorTableV1,
}

impl DescriptorLineageV1 {
    pub const fn new(table: DeviceDescriptorTableV1) -> Self {
        Self { table }
    }

    pub const fn table(&self) -> &DeviceDescriptorTableV1 {
        &self.table
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        encode_device_descriptor_table_v1(&self.table)
            .expect("validated descriptor table must retain a canonical encoding")
    }

    pub const fn grants_compiler_origin_authority(&self) -> bool {
        false
    }
}

/// Bounded canonical inputs needed by a future recovered Worker V2 admission.
///
/// Construction establishes canonical structural closure and joins the proof
/// kernel, source, and executable identities to the manifest. It does not
/// authenticate proof/compiler provenance, inspect the finalized HSACO, or
/// establish publication currentness.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV2LoadEnvelopeV1 {
    container: ArtifactContainerV1,
    bundle_index: BundleIndexV1,
    direct_link_evidence: DirectLinkBundleEvidenceV1,
    descriptor_lineage: DescriptorLineageV1,
    proof_records: Vec<ProofRecordV1>,
    raw_hsaco: ExactRawHsacoV1,
    published_claim: DurablePublishedHsacoClaimV1,
}

impl WorkerV2LoadEnvelopeV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        container: ArtifactContainerV1,
        bundle_index: BundleIndexV1,
        direct_link_evidence: DirectLinkBundleEvidenceV1,
        descriptor_lineage: DescriptorLineageV1,
        mut proof_records: Vec<ProofRecordV1>,
        raw_hsaco: ExactRawHsacoV1,
        published_claim: DurablePublishedHsacoClaimV1,
    ) -> Result<Self, EnvelopeValidationError> {
        validate_raw_hsaco(raw_hsaco.identity, &raw_hsaco.bytes)?;
        let derived = BundleIndexV1::from_containers(std::slice::from_ref(&container))?;
        if derived != bundle_index {
            return Err(EnvelopeValidationError::BundleDoesNotMatchContainer);
        }
        if direct_link_evidence.bindings().len() != 1 {
            return Err(EnvelopeValidationError::DirectLinkBindingCount {
                actual: direct_link_evidence.bindings().len(),
            });
        }

        let binding = &direct_link_evidence.bindings()[0];
        let source = DirectLinkBindingSourceV1::new(&container, binding.expectation().clone());
        let validated = direct_link_evidence.validate_against(
            &bundle_index,
            &[&container],
            std::slice::from_ref(&source),
        )?;
        validate_payloads(&container, binding.expectation(), &raw_hsaco)?;
        validate_descriptor(&container, descriptor_lineage.table())?;
        canonicalize_and_validate_proofs(&container, &mut proof_records)?;
        validate_publication_claim(&container, &validated, &published_claim)?;

        Ok(Self {
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
            published_claim,
        })
    }

    pub const fn container(&self) -> &ArtifactContainerV1 {
        &self.container
    }

    pub const fn bundle_index(&self) -> &BundleIndexV1 {
        &self.bundle_index
    }

    pub const fn direct_link_evidence(&self) -> &DirectLinkBundleEvidenceV1 {
        &self.direct_link_evidence
    }

    pub const fn descriptor_lineage(&self) -> &DescriptorLineageV1 {
        &self.descriptor_lineage
    }

    pub fn proof_records(&self) -> &[ProofRecordV1] {
        &self.proof_records
    }

    pub const fn raw_hsaco(&self) -> &ExactRawHsacoV1 {
        &self.raw_hsaco
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV1 {
        &self.published_claim
    }

    pub fn finalized_payload(&self) -> &[u8] {
        let identity = self.direct_link_evidence.bindings()[0]
            .expectation()
            .finalized_payload_identity()
            .digest();
        self.container
            .payloads()
            .iter()
            .find(|payload| payload.digest() == identity)
            .expect("validated envelope must retain the finalized payload")
            .bytes()
    }

    pub fn finalized_payload_identity(&self) -> PayloadDigest {
        self.direct_link_evidence.bindings()[0]
            .expectation()
            .finalized_payload_identity()
            .digest()
    }

    pub const fn grants_currentness_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn validate_raw_hsaco(
    identity: PayloadDigest,
    bytes: &[u8],
) -> Result<(), EnvelopeValidationError> {
    if bytes.is_empty() {
        return Err(EnvelopeValidationError::EmptyRawHsaco);
    }
    if bytes.len() > MAX_WORKER_V2_RAW_HSACO_BYTES {
        return Err(EnvelopeValidationError::RawHsacoTooLarge {
            max: MAX_WORKER_V2_RAW_HSACO_BYTES,
        });
    }
    if identity.algorithm() != DigestAlgorithm::Sha256 {
        return Err(EnvelopeValidationError::UnsupportedDigestAlgorithm { field: "raw HSACO" });
    }
    identity
        .verify(bytes)
        .map_err(|_| EnvelopeValidationError::RawHsacoDigestMismatch)
}

fn validate_payloads(
    container: &ArtifactContainerV1,
    expectation: &fe2o3_artifacts::DirectLinkBindingExpectationV1,
    raw_hsaco: &ExactRawHsacoV1,
) -> Result<(), EnvelopeValidationError> {
    let linked = expectation.linked_output_identity().digest();
    let finalized = expectation.finalized_payload_identity().digest();
    for (field, digest) in [("linked output", linked), ("finalized payload", finalized)] {
        if digest.algorithm() != DigestAlgorithm::Sha256 {
            return Err(EnvelopeValidationError::UnsupportedDigestAlgorithm { field });
        }
    }
    if linked != raw_hsaco.identity {
        return Err(EnvelopeValidationError::PublicationClaimMismatch(
            PublicationClaimFieldV1::LinkedOutput,
        ));
    }
    let code_object = container
        .manifest()
        .code_objects()
        .iter()
        .find(|object| object.digest() == finalized.bytes())
        .ok_or(EnvelopeValidationError::MissingFinalizedPayload)?;
    if code_object.format() != CodeObjectFormat::NativeExecutable {
        return Err(EnvelopeValidationError::FinalizedPayloadIsNotNative);
    }
    let payload = container
        .payloads()
        .iter()
        .find(|payload| payload.digest() == finalized)
        .ok_or(EnvelopeValidationError::MissingFinalizedPayload)?;
    finalized
        .verify(payload.bytes())
        .map_err(|_| EnvelopeValidationError::MissingFinalizedPayload)?;
    if container
        .manifest()
        .kernels()
        .iter()
        .any(|kernel| kernel.code_object_digest() != finalized.bytes())
    {
        return Err(EnvelopeValidationError::FinalizedPayloadNotUsedByEveryKernel);
    }
    Ok(())
}

fn validate_descriptor(
    container: &ArtifactContainerV1,
    table: &DeviceDescriptorTableV1,
) -> Result<(), EnvelopeValidationError> {
    let manifest = container.manifest();
    if table.device_target().to_string() != manifest.target().architecture().as_str()
        || manifest.target().triple().as_str() != "amdgcn-amd-amdhsa"
        || manifest.target().pointer_width() != PointerWidth::Bits64
        || manifest.target().endianness() != Endianness::Little
    {
        return Err(EnvelopeValidationError::DescriptorTargetMismatch);
    }
    if table.canonical_code_object_digest().as_bytes() == &[0; 32] {
        return Err(EnvelopeValidationError::UnfinalizedDescriptorLineage);
    }
    if table.kernels().len() != manifest.kernels().len() {
        return Err(EnvelopeValidationError::DescriptorKernelCountMismatch);
    }
    for (descriptor, kernel) in table.kernels().iter().zip(manifest.kernels()) {
        for (matches, field) in [
            (
                descriptor.kernel_id().as_bytes() == kernel.kernel_id().as_bytes(),
                "ID",
            ),
            (
                descriptor.logical_name().as_str() == kernel.name().as_str(),
                "logical name",
            ),
            (
                descriptor.entry_name().as_str() == kernel.symbol().as_str(),
                "entry symbol",
            ),
            (
                descriptor.source_evidence().digest().as_bytes()
                    == kernel.source_digest().as_bytes(),
                "source digest",
            ),
            (
                descriptor.executable_ir_evidence().digest().as_bytes()
                    == kernel.executable_digest().as_bytes(),
                "executable digest",
            ),
        ] {
            if !matches {
                return Err(EnvelopeValidationError::DescriptorKernelMismatch { field });
            }
        }
    }
    Ok(())
}

fn canonicalize_and_validate_proofs(
    container: &ArtifactContainerV1,
    proof_records: &mut [ProofRecordV1],
) -> Result<(), EnvelopeValidationError> {
    if proof_records.len() != container.manifest().kernels().len() {
        return Err(EnvelopeValidationError::ProofCountMismatch);
    }
    proof_records.sort_unstable_by_key(|record| record.target().artifact().kernel_id());
    if proof_records.windows(2).any(|pair| {
        pair[0].target().artifact().kernel_id() == pair[1].target().artifact().kernel_id()
    }) {
        return Err(EnvelopeValidationError::DuplicateProofKernel);
    }
    let total = proof_records.iter().try_fold(0usize, |total, record| {
        total.checked_add(record.to_bytes().len())
    });
    if total.is_none_or(|total| total > MAX_WORKER_V2_PROOF_EVIDENCE_BYTES) {
        return Err(EnvelopeValidationError::ProofEvidenceTooLarge {
            max: MAX_WORKER_V2_PROOF_EVIDENCE_BYTES,
        });
    }
    for (record, kernel) in proof_records.iter().zip(container.manifest().kernels()) {
        let artifact = record.target().artifact();
        let kernel_id = artifact.kernel_id();
        if kernel_id.algorithm() != DigestAlgorithm::Sha256
            || kernel_id.bytes().as_bytes() != kernel.kernel_id().as_bytes()
        {
            return Err(EnvelopeValidationError::ProofKernelSetMismatch);
        }
        for (proof, manifest, field) in [
            (
                artifact.source_tree_digest(),
                kernel.source_digest(),
                "source identity",
            ),
            (
                artifact.executable_digest(),
                kernel.executable_digest(),
                "executable identity",
            ),
        ] {
            if proof.algorithm() != DigestAlgorithm::Sha256
                || proof.bytes().as_bytes() != manifest.as_bytes()
            {
                return Err(EnvelopeValidationError::ProofManifestMismatch { field });
            }
        }
    }
    Ok(())
}

fn validate_publication_claim(
    container: &ArtifactContainerV1,
    validated: &fe2o3_artifacts::ValidatedDirectLinkBundleEvidenceV1<'_>,
    claim: &DurablePublishedHsacoClaimV1,
) -> Result<(), EnvelopeValidationError> {
    let plan = claim.plan();
    let package = CallerClaimedPackageIdentityV1::new(plan.scope().package());
    let scope =
        ManifestClaimDerivedLinkPublicationScopeV1::derive(package, validated, 0, container)
            .map_err(|_| EnvelopeValidationError::PublicationBridge)?;
    let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
        plan.attempt(),
        scope,
        validated,
        0,
    )
    .map_err(|_| EnvelopeValidationError::PublicationBridge)?;
    let expected_scope = bridge
        .non_authoritative_diagnostics()
        .descriptive_scope_claim();
    let checks = [
        (
            plan.scope() == expected_scope,
            PublicationClaimFieldV1::Scope,
        ),
        (
            plan.request() == bridge.request_identity(),
            PublicationClaimFieldV1::Request,
        ),
        (
            plan.worker() == bridge.worker_identity(),
            PublicationClaimFieldV1::Worker,
        ),
        (
            plan.response() == bridge.response_identity(),
            PublicationClaimFieldV1::Response,
        ),
        (
            plan.linked_output() == bridge.linked_output_identity(),
            PublicationClaimFieldV1::LinkedOutput,
        ),
        (
            plan.finalization() == bridge.finalization_identity(),
            PublicationClaimFieldV1::Finalization,
        ),
        (
            plan.finalized_output() == bridge.finalized_output_identity(),
            PublicationClaimFieldV1::FinalizedOutput,
        ),
        (
            plan.publication() == bridge.publication_identity(),
            PublicationClaimFieldV1::Publication,
        ),
    ];
    for (matches, field) in checks {
        if !matches {
            return Err(EnvelopeValidationError::PublicationClaimMismatch(field));
        }
    }
    let evidence_digest = validated
        .evidence()
        .digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM);
    if evidence_digest.algorithm() != DigestAlgorithm::Sha256 {
        return Err(EnvelopeValidationError::UnsupportedDigestAlgorithm {
            field: "direct-link evidence",
        });
    }
    if claim.upstream_evidence().as_bytes() != *evidence_digest.bytes().as_bytes() {
        return Err(EnvelopeValidationError::PublicationClaimMismatch(
            PublicationClaimFieldV1::UpstreamEvidence,
        ));
    }
    Ok(())
}

impl From<fe2o3_artifacts::BundleValidationError> for EnvelopeValidationError {
    fn from(_: fe2o3_artifacts::BundleValidationError) -> Self {
        Self::BundleDoesNotMatchContainer
    }
}
