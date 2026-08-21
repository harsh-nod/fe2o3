use fe2o3_artifact_transaction::{
    DurableLinkPublicationPlanV1, DurablePublishedHsacoClaimV1,
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, UpstreamCodeObjectEvidenceIdentityV1,
};
use fe2o3_artifacts::{
    AbiKind, Access, AliasClass, ArgumentOwnership, ArtifactContainerV1, BlockSize, BundleIndexV1,
    CallerClaimedPackageIdentityV1, Capability, CodeObjectFormat,
    DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestAlgorithm, DirectLinkBindingSourceV1,
    DirectLinkBundleEvidenceV1, Endianness, MAX_BUNDLE_INDEX_BYTES, MAX_CONTAINER_BYTES,
    MAX_DIRECT_LINK_EVIDENCE_BYTES, ManifestClaimDerivedLinkPublicationScopeV1,
    ManifestClaimDirectLinkPublicationBridgeV1, PayloadDigest, PointerWidth, ProofRecordV1,
    ScalarType,
};
use fe2o3_hsaco_finalize::inspect_finalized;
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CanonicalCodeObjectDigest, CapabilityV1,
    DeviceDescriptorTableV1, MAX_DESCRIPTOR_TABLE_BYTES, OwnershipSemantics,
    PhysicalAbiComponentKind, ScalarTypeV1, encode_device_descriptor_table_v1,
};
use sha2::{Digest, Sha256};

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
const LOAD_ENVELOPE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-LOAD-ENVELOPE/V1\0";

/// SHA-256 identity of one complete canonical Worker V2 load envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2LoadEnvelopeIdentityV1([u8; 32]);

impl WorkerV2LoadEnvelopeIdentityV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

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
    components: WorkerV2EnvelopeComponentsV1,
    published_claim: DurablePublishedHsacoClaimV1,
}

/// Schema-neutral validated component closure shared by V1 and protected V2 envelopes.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct WorkerV2EnvelopeComponentsV1 {
    container: ArtifactContainerV1,
    bundle_index: BundleIndexV1,
    direct_link_evidence: DirectLinkBundleEvidenceV1,
    descriptor_lineage: DescriptorLineageV1,
    proof_records: Vec<ProofRecordV1>,
    raw_hsaco: ExactRawHsacoV1,
}

impl WorkerV2LoadEnvelopeV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        container: ArtifactContainerV1,
        bundle_index: BundleIndexV1,
        direct_link_evidence: DirectLinkBundleEvidenceV1,
        descriptor_lineage: DescriptorLineageV1,
        proof_records: Vec<ProofRecordV1>,
        raw_hsaco: ExactRawHsacoV1,
        published_claim: DurablePublishedHsacoClaimV1,
    ) -> Result<Self, EnvelopeValidationError> {
        let claim = PublicationClaimViewV1::new(
            published_claim.plan(),
            published_claim.upstream_evidence(),
        );
        let components = WorkerV2EnvelopeComponentsV1::new(
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
            claim,
        )?;

        Ok(Self {
            components,
            published_claim,
        })
    }

    pub const fn container(&self) -> &ArtifactContainerV1 {
        self.components.container()
    }

    pub const fn bundle_index(&self) -> &BundleIndexV1 {
        self.components.bundle_index()
    }

    pub const fn direct_link_evidence(&self) -> &DirectLinkBundleEvidenceV1 {
        self.components.direct_link_evidence()
    }

    pub const fn descriptor_lineage(&self) -> &DescriptorLineageV1 {
        self.components.descriptor_lineage()
    }

    pub fn proof_records(&self) -> &[ProofRecordV1] {
        self.components.proof_records()
    }

    pub const fn raw_hsaco(&self) -> &ExactRawHsacoV1 {
        self.components.raw_hsaco()
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV1 {
        &self.published_claim
    }

    pub fn identity(&self) -> WorkerV2LoadEnvelopeIdentityV1 {
        let mut digest = Sha256::new();
        digest.update(LOAD_ENVELOPE_IDENTITY_DOMAIN);
        digest.update(self.to_bytes());
        WorkerV2LoadEnvelopeIdentityV1(digest.finalize().into())
    }

    pub fn finalized_payload(&self) -> &[u8] {
        self.components.finalized_payload()
    }

    pub fn finalized_payload_identity(&self) -> PayloadDigest {
        self.components.finalized_payload_identity()
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

impl WorkerV2EnvelopeComponentsV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        container: ArtifactContainerV1,
        bundle_index: BundleIndexV1,
        direct_link_evidence: DirectLinkBundleEvidenceV1,
        descriptor_lineage: DescriptorLineageV1,
        proof_records: Vec<ProofRecordV1>,
        raw_hsaco: ExactRawHsacoV1,
        claim: PublicationClaimViewV1,
    ) -> Result<Self, EnvelopeValidationError> {
        Self::new_with_descriptor_profile(
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
            claim,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_protected_v2(
        container: ArtifactContainerV1,
        bundle_index: BundleIndexV1,
        direct_link_evidence: DirectLinkBundleEvidenceV1,
        descriptor_lineage: DescriptorLineageV1,
        proof_records: Vec<ProofRecordV1>,
        raw_hsaco: ExactRawHsacoV1,
        claim: PublicationClaimViewV1,
    ) -> Result<Self, EnvelopeValidationError> {
        Self::new_with_descriptor_profile(
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
            claim,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_descriptor_profile(
        container: ArtifactContainerV1,
        bundle_index: BundleIndexV1,
        direct_link_evidence: DirectLinkBundleEvidenceV1,
        descriptor_lineage: DescriptorLineageV1,
        mut proof_records: Vec<ProofRecordV1>,
        raw_hsaco: ExactRawHsacoV1,
        claim: PublicationClaimViewV1,
        protected_v2: bool,
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
        if protected_v2 {
            validate_protected_descriptor_semantics(&container, descriptor_lineage.table())?;
        }
        canonicalize_and_validate_proofs(&container, &mut proof_records)?;
        validate_publication_claim(&container, &validated, claim)?;

        Ok(Self {
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
        })
    }

    pub(crate) const fn container(&self) -> &ArtifactContainerV1 {
        &self.container
    }

    pub(crate) const fn bundle_index(&self) -> &BundleIndexV1 {
        &self.bundle_index
    }

    pub(crate) const fn direct_link_evidence(&self) -> &DirectLinkBundleEvidenceV1 {
        &self.direct_link_evidence
    }

    pub(crate) const fn descriptor_lineage(&self) -> &DescriptorLineageV1 {
        &self.descriptor_lineage
    }

    pub(crate) fn proof_records(&self) -> &[ProofRecordV1] {
        &self.proof_records
    }

    pub(crate) const fn raw_hsaco(&self) -> &ExactRawHsacoV1 {
        &self.raw_hsaco
    }

    pub(crate) fn finalized_payload(&self) -> &[u8] {
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

    pub(crate) fn finalized_payload_identity(&self) -> PayloadDigest {
        self.direct_link_evidence.bindings()[0]
            .expectation()
            .finalized_payload_identity()
            .digest()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PublicationClaimViewV1 {
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
}

impl PublicationClaimViewV1 {
    pub(crate) const fn new(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    ) -> Self {
        Self {
            plan,
            upstream_evidence,
        }
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

pub(crate) fn validate_payloads(
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

pub(crate) fn validate_descriptor(
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

fn validate_protected_descriptor_semantics(
    container: &ArtifactContainerV1,
    table: &DeviceDescriptorTableV1,
) -> Result<(), EnvelopeValidationError> {
    let Some(code_object_digest) = container
        .manifest()
        .kernels()
        .first()
        .map(|kernel| kernel.code_object_digest())
    else {
        return Err(EnvelopeValidationError::DescriptorKernelMismatch {
            field: "canonical code-object digest",
        });
    };
    let Some(finalized_payload) = container
        .payloads()
        .iter()
        .find(|payload| payload.digest().bytes() == code_object_digest)
    else {
        return Err(EnvelopeValidationError::DescriptorKernelMismatch {
            field: "canonical code-object digest",
        });
    };
    let descriptor_matches_payload = match inspect_finalized(finalized_payload.bytes()) {
        Ok(inspection) => {
            inspection.digest() == table.canonical_code_object_digest()
                && inspection.descriptor_table() == table
        }
        Err(_) => {
            table.canonical_code_object_digest()
                == CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(
                    finalized_payload.bytes(),
                )
        }
    };
    if !descriptor_matches_payload {
        return Err(EnvelopeValidationError::DescriptorKernelMismatch {
            field: "canonical code-object digest",
        });
    }
    for (descriptor, kernel) in table.kernels().iter().zip(container.manifest().kernels()) {
        let capabilities_match = descriptor.capabilities().len()
            == kernel.required_capabilities().len()
            && descriptor
                .capabilities()
                .iter()
                .zip(kernel.required_capabilities())
                .all(|(descriptor, manifest)| capability_matches(*descriptor, *manifest));
        if !capabilities_match {
            return Err(EnvelopeValidationError::DescriptorKernelMismatch {
                field: "capability closure",
            });
        }

        let manifest_abi = kernel.abi();
        let descriptor_abi = descriptor.abi_layout();
        if u64::from(descriptor_abi.explicit_argument_size()) != manifest_abi.size()
            || descriptor_abi.kernarg_segment_alignment() != manifest_abi.alignment()
            || descriptor.arguments().len() != manifest_abi.fields().len()
        {
            return Err(EnvelopeValidationError::DescriptorKernelMismatch {
                field: "physical ABI",
            });
        }
        for (index, (argument, field)) in descriptor
            .arguments()
            .iter()
            .zip(manifest_abi.fields())
            .enumerate()
        {
            if usize::from(argument.source_index()) != index
                || argument.name().as_str() != field.name().as_str()
                || !ownership_matches(argument.ownership(), field.ownership())
                || !access_matches(argument.access(), field.access())
                || !alias_matches(argument.alias(), field.alias_class())
                || !logical_argument_layout_matches(table, argument, field)
            {
                return Err(EnvelopeValidationError::DescriptorKernelMismatch {
                    field: "logical argument or type/layout closure",
                });
            }
        }

        let manifest_launch = kernel.launch();
        let descriptor_launch = descriptor.launch();
        if descriptor_launch.rank() != manifest_launch.rank()
            || !block_size_matches(descriptor_launch.block_size(), manifest_launch.block_size())
            || !dimensions_match(descriptor_launch.max_grid(), manifest_launch.max_grid())
            || descriptor_launch.static_shared_memory_bytes()
                != manifest_launch.static_shared_memory_bytes()
            || descriptor_launch.max_dynamic_shared_memory_bytes()
                != manifest_launch.max_dynamic_shared_memory_bytes()
        {
            return Err(EnvelopeValidationError::DescriptorKernelMismatch {
                field: "launch and shared-memory resource contract",
            });
        }
    }
    Ok(())
}

fn capability_matches(descriptor: CapabilityV1, manifest: Capability) -> bool {
    matches!(
        (descriptor, manifest),
        (CapabilityV1::Subgroup, Capability::Subgroup)
            | (CapabilityV1::Ballot, Capability::Ballot)
            | (CapabilityV1::Shuffle, Capability::Shuffle)
            | (CapabilityV1::WorkgroupMemory, Capability::WorkgroupMemory)
            | (CapabilityV1::MatrixMultiply, Capability::MatrixMultiply)
            | (CapabilityV1::AsyncCopy, Capability::AsyncCopy)
            | (CapabilityV1::Atomics, Capability::Atomics)
            | (CapabilityV1::AmdWave, Capability::AmdWave)
            | (CapabilityV1::AmdMfma, Capability::AmdMfma)
            | (CapabilityV1::AmdWmma, Capability::AmdWmma)
            | (CapabilityV1::AmdDsPermute, Capability::AmdDsPermute)
    )
}

fn ownership_matches(descriptor: OwnershipSemantics, manifest: ArgumentOwnership) -> bool {
    matches!(
        (descriptor, manifest),
        (OwnershipSemantics::ByValue, ArgumentOwnership::ByValue)
            | (
                OwnershipSemantics::SharedBorrow,
                ArgumentOwnership::SharedBorrow
            )
            | (
                OwnershipSemantics::UniqueBorrow,
                ArgumentOwnership::UniqueBorrow
            )
    )
}

fn access_matches(descriptor: AccessMode, manifest: Access) -> bool {
    matches!(
        (descriptor, manifest),
        (AccessMode::ByValue, Access::ByValue)
            | (AccessMode::ReadOnly, Access::ReadOnly)
            | (AccessMode::WriteOnly, Access::WriteOnly)
            | (AccessMode::ReadWrite, Access::ReadWrite)
    )
}

fn alias_matches(descriptor: AliasSemantics, manifest: AliasClass) -> bool {
    matches!(
        (descriptor, manifest),
        (AliasSemantics::Value, AliasClass::Value)
            | (AliasSemantics::SharedReadOnly, AliasClass::SharedReadOnly)
            | (AliasSemantics::Exclusive, AliasClass::Exclusive)
    )
}

fn logical_argument_layout_matches(
    table: &DeviceDescriptorTableV1,
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
    field: &fe2o3_artifacts::AbiField,
) -> bool {
    let Some(source_type) = table
        .type_records()
        .iter()
        .find(|record| record.identity() == argument.source_type())
    else {
        return false;
    };
    let Some(device_layout) = table
        .layout_records()
        .iter()
        .find(|record| record.identity() == argument.device_layout())
    else {
        return false;
    };
    if argument.source_type().as_bytes() != field.type_identity().rust_type().bytes().as_bytes()
        || argument.device_layout().as_bytes() != field.type_identity().layout().bytes().as_bytes()
    {
        return false;
    }
    let Ok(offset) = u32::try_from(field.offset()) else {
        return false;
    };
    let components = argument.physical_components().collect::<Vec<_>>();
    match field.kind() {
        AbiKind::Scalar(manifest_scalar) => {
            let descriptor_scalar = scalar_type_v1(manifest_scalar);
            source_type.descriptor().is_scalar()
                && device_layout.descriptor().size_bytes() == descriptor_scalar.size_bytes()
                && device_layout.descriptor().alignment_bytes()
                    == descriptor_scalar.alignment_bytes()
                && source_type.descriptor().scalar_type() == descriptor_scalar
                && device_layout.descriptor().scalar_type() == descriptor_scalar
                && components.as_slice()
                    == [(
                        PhysicalAbiComponentKind::ScalarByValue(descriptor_scalar),
                        offset,
                        descriptor_scalar.size_bytes(),
                        descriptor_scalar.alignment_bytes(),
                    )]
        }
        AbiKind::Slice {
            element_size,
            element_alignment,
        } => {
            let Some(length_offset) = offset.checked_add(8) else {
                return false;
            };
            let descriptor_scalar = source_type.descriptor().scalar_type();
            let kind_matches = match field.ownership() {
                ArgumentOwnership::SharedBorrow => source_type.descriptor().is_shared_slice(),
                ArgumentOwnership::UniqueBorrow => source_type.descriptor().is_disjoint_slice(),
                _ => false,
            };
            kind_matches
                && device_layout.descriptor().scalar_type() == descriptor_scalar
                && u64::from(descriptor_scalar.size_bytes()) == element_size
                && u32::from(descriptor_scalar.alignment_bytes()) == element_alignment
                && field.size() == 16
                && field.alignment() == 8
                && components.as_slice()
                    == [
                        (PhysicalAbiComponentKind::GlobalPointer, offset, 8, 8),
                        (
                            PhysicalAbiComponentKind::SliceLengthU64,
                            length_offset,
                            8,
                            8,
                        ),
                    ]
        }
        AbiKind::Pointer { .. } => false,
    }
}

const fn scalar_type_v1(value: ScalarType) -> ScalarTypeV1 {
    match value {
        ScalarType::I8 => ScalarTypeV1::I8,
        ScalarType::U8 => ScalarTypeV1::U8,
        ScalarType::I16 => ScalarTypeV1::I16,
        ScalarType::U16 => ScalarTypeV1::U16,
        ScalarType::I32 => ScalarTypeV1::I32,
        ScalarType::U32 => ScalarTypeV1::U32,
        ScalarType::I64 => ScalarTypeV1::I64,
        ScalarType::U64 => ScalarTypeV1::U64,
        ScalarType::F16 => ScalarTypeV1::F16,
        ScalarType::F32 => ScalarTypeV1::F32,
        ScalarType::F64 => ScalarTypeV1::F64,
    }
}

fn block_size_matches(descriptor: BlockSizeV1, manifest: BlockSize) -> bool {
    match (descriptor, manifest) {
        (BlockSizeV1::Any, BlockSize::Any) => true,
        (BlockSizeV1::Exact(left), BlockSize::Exact(right))
        | (BlockSizeV1::AtMost(left), BlockSize::AtMost(right)) => dimensions_match(left, right),
        _ => false,
    }
}

fn dimensions_match(
    descriptor: fe2o3_kernel_descriptor::DimensionsV1,
    manifest: fe2o3_artifacts::Dimensions,
) -> bool {
    descriptor.x() == manifest.x()
        && descriptor.y() == manifest.y()
        && descriptor.z() == manifest.z()
}

pub(crate) fn canonicalize_and_validate_proofs(
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
    claim: PublicationClaimViewV1,
) -> Result<(), EnvelopeValidationError> {
    let plan = claim.plan;
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
    if claim.upstream_evidence.as_bytes() != *evidence_digest.bytes().as_bytes() {
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
