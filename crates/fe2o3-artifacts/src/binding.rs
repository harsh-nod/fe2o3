use std::fmt;

use crate::encode::{
    access_tag, address_space_tag, capability_tag, code_object_format_tag, endianness_tag,
    mutability_tag, pointer_width_tag, scalar_tag,
};
use crate::proof::{canonicalize_configuration, canonicalize_trusted_items};
use crate::proof_encode::digest_algorithm_tag;
use crate::{
    AbiKind, AbiLayout, BlockSize, ConfigurationEntry, DigestAlgorithm, DigestBytes, KernelEntry,
    LaunchContract, ManifestV1, MeasuredToolIdentity, Name, PayloadDigest, ProofArtifactIdentity,
    ProofExecutionIdentity, ProofOutcome, ProofProperty, ProofRecordV1, ProofTargetIdentity,
    SourceContractIdentity, TrustedItem, ValidationError,
};

const ARTIFACT_ENVIRONMENT_MAGIC: [u8; 8] = *b"FE2O3EN\0";
const ARTIFACT_SELECTION_MAGIC: [u8; 8] = *b"FE2O3SL\0";
const ARTIFACT_CONTRACT_MAGIC: [u8; 8] = *b"FE2O3KC\0";
pub const PROOF_IDENTITY_VERSION: u16 = 1;

/// Complete property set required for V1 evidence matching.
///
/// Presence of these self-reported properties never promotes assurance.
pub const V1_REQUIRED_PROPERTIES: [ProofProperty; 7] = [
    ProofProperty::Bounds,
    ProofProperty::AddressOverflowFreedom,
    ProofProperty::MemorySafety,
    ProofProperty::Initialization,
    ProofProperty::RaceFreedom,
    ProofProperty::LaunchValidity,
    ProofProperty::FunctionalCorrectness,
];

impl ManifestV1 {
    /// Reconstructs the artifact-owned portion of a proof target from explicit,
    /// algorithm-tagged identities.
    ///
    /// The manifest stores opaque digest bytes, so this method requires tagged
    /// kernel, source, executable, and code-object identities and rejects any
    /// byte mismatch instead of assigning an algorithm to manifest bytes. The
    /// instance, crate graph, and source-contract digests remain explicit inputs.
    /// Compiler and producer measurements must identify the same named versions
    /// as the manifest. Matching does not authenticate any supplied measurement.
    #[allow(clippy::too_many_arguments)]
    pub fn proof_target(
        &self,
        kernel_id: PayloadDigest,
        instance_digest: PayloadDigest,
        source_tree_digest: PayloadDigest,
        crate_graph_digest: PayloadDigest,
        executable_digest: PayloadDigest,
        code_object_digest: PayloadDigest,
        source_contracts: SourceContractIdentity,
        compiler: &MeasuredToolIdentity,
        artifact_producer: &MeasuredToolIdentity,
        derived_identity_algorithm: DigestAlgorithm,
    ) -> Result<ProofTargetIdentity, ProofTargetError> {
        let kernel = self
            .kernels()
            .binary_search_by_key(&kernel_id.bytes(), KernelEntry::kernel_id)
            .ok()
            .map(|index| &self.kernels()[index])
            .ok_or(ProofTargetError::UnknownKernel(kernel_id.bytes()))?;
        require_manifest_digest("source tree", source_tree_digest, kernel.source_digest())?;
        require_manifest_digest(
            "executable semantic",
            executable_digest,
            kernel.executable_digest(),
        )?;
        require_manifest_digest(
            "code object",
            code_object_digest,
            kernel.code_object_digest(),
        )?;
        require_manifest_tool(
            "compiler",
            compiler,
            self.compiler().name().as_str(),
            self.compiler().version().as_str(),
        )?;
        require_manifest_tool(
            "artifact producer",
            artifact_producer,
            self.producer().name().as_str(),
            self.producer().version().as_str(),
        )?;
        let code_object = self
            .code_objects()
            .binary_search_by_key(&kernel.code_object_digest(), |object| object.digest())
            .ok()
            .map(|index| &self.code_objects()[index])
            .expect("validated manifest closes kernel code-object references");

        Ok(ProofTargetIdentity::new(
            ProofArtifactIdentity::new(
                kernel_id,
                instance_digest,
                source_tree_digest,
                crate_graph_digest,
                executable_digest,
                derived_identity_algorithm.calculate(&environment_bytes(
                    self,
                    compiler,
                    artifact_producer,
                )),
                derived_identity_algorithm.calculate(&selection_bytes(
                    kernel,
                    code_object,
                    code_object_digest,
                )),
                derived_identity_algorithm.calculate(&contract_bytes(kernel)),
            ),
            source_contracts,
        ))
    }
}

/// Exact identities and policy expected by artifact finalization.
///
/// Matching proves equality with caller-supplied evidence only. This type does
/// not authenticate a Verus run and cannot produce a `Verified` assurance.
/// Invocation authentication alone is also insufficient: only a future audited
/// driver that authoritatively derives the complete property set and trusted
/// escape inventory from source and verifier output, then authenticates the
/// whole canonical record, may construct a stronger private type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofMatchPolicy {
    target: ProofTargetIdentity,
    configuration: Vec<ConfigurationEntry>,
    execution: ProofExecutionIdentity,
    approved_trusted_items: Vec<TrustedItem>,
}

impl ProofMatchPolicy {
    pub fn new(
        target: ProofTargetIdentity,
        mut configuration: Vec<ConfigurationEntry>,
        execution: ProofExecutionIdentity,
        mut approved_trusted_items: Vec<TrustedItem>,
    ) -> Result<Self, ValidationError> {
        canonicalize_configuration(&mut configuration)?;
        canonicalize_trusted_items(&mut approved_trusted_items)?;
        Ok(Self {
            target,
            configuration,
            execution,
            approved_trusted_items,
        })
    }

    pub const fn target(&self) -> ProofTargetIdentity {
        self.target
    }

    pub fn configuration(&self) -> &[ConfigurationEntry] {
        &self.configuration
    }

    pub const fn execution(&self) -> &ProofExecutionIdentity {
        &self.execution
    }

    pub fn approved_trusted_items(&self) -> &[TrustedItem] {
        &self.approved_trusted_items
    }

    /// Matches a complete proof result without elevating its assurance level.
    pub fn match_record(
        &self,
        record: ProofRecordV1,
        digest_algorithm: DigestAlgorithm,
    ) -> Result<MatchedProofEvidenceV1, ProofMatchError> {
        compare_target(self.target, record.target())?;
        if self.configuration != record.configuration() {
            return Err(ProofMatchError::IdentityMismatch("configuration"));
        }
        if self.execution.model() != record.execution().model() {
            return Err(ProofMatchError::IdentityMismatch("verification model"));
        }
        if self.execution.verifier() != record.execution().verifier() {
            return Err(ProofMatchError::IdentityMismatch("verifier"));
        }
        if self.execution.solver() != record.execution().solver() {
            return Err(ProofMatchError::IdentityMismatch("solver"));
        }
        if self.execution.evidence_recorder() != record.execution().evidence_recorder() {
            return Err(ProofMatchError::IdentityMismatch("evidence recorder"));
        }
        if self.execution.invocation_digest() != record.execution().invocation_digest() {
            return Err(ProofMatchError::IdentityMismatch("proof invocation"));
        }
        if record.outcome() != ProofOutcome::Proved {
            return Err(ProofMatchError::ProofNotComplete(record.outcome()));
        }
        for property in V1_REQUIRED_PROPERTIES {
            if record.proved_properties().binary_search(&property).is_err() {
                return Err(ProofMatchError::MissingProperty(property));
            }
        }
        if self.approved_trusted_items != record.trusted_items() {
            return Err(ProofMatchError::TrustedItemsMismatch);
        }

        let record_digest = record.digest(digest_algorithm);
        Ok(MatchedProofEvidenceV1 {
            record,
            record_digest,
        })
    }
}

/// Structurally valid proof evidence that exactly matches a finalization policy.
///
/// This is deliberately not a verified artifact and is never sufficient input
/// for assurance promotion. Invocation authentication alone is insufficient.
/// A future audited driver must authoritatively derive the complete property set
/// and trusted escape inventory from source and verifier output and authenticate
/// the whole canonical record before privately constructing any stronger type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchedProofEvidenceV1 {
    record: ProofRecordV1,
    record_digest: PayloadDigest,
}

impl MatchedProofEvidenceV1 {
    pub const fn record(&self) -> &ProofRecordV1 {
        &self.record
    }

    pub const fn record_digest(&self) -> PayloadDigest {
        self.record_digest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofTargetError {
    UnknownKernel(DigestBytes),
    ManifestDigestMismatch(&'static str),
    ManifestToolMismatch(&'static str),
}

impl fmt::Display for ProofTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKernel(_) => write!(f, "proof target names an unknown kernel"),
            Self::ManifestDigestMismatch(field) => {
                write!(f, "proof target {field} digest does not match the manifest")
            }
            Self::ManifestToolMismatch(field) => {
                write!(f, "measured {field} identity does not match the manifest")
            }
        }
    }
}

impl std::error::Error for ProofTargetError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofMatchError {
    IdentityMismatch(&'static str),
    ProofNotComplete(ProofOutcome),
    MissingProperty(ProofProperty),
    TrustedItemsMismatch,
}

impl fmt::Display for ProofMatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityMismatch(field) => write!(f, "proof {field} identity does not match"),
            Self::ProofNotComplete(outcome) => {
                write!(f, "proof outcome {outcome:?} is not complete")
            }
            Self::MissingProperty(property) => {
                write!(f, "proof does not establish required property {property:?}")
            }
            Self::TrustedItemsMismatch => {
                write!(f, "proof trusted items do not match the approved policy")
            }
        }
    }
}

impl std::error::Error for ProofMatchError {}

fn compare_target(
    expected: ProofTargetIdentity,
    actual: ProofTargetIdentity,
) -> Result<(), ProofMatchError> {
    let expected_artifact = expected.artifact();
    let actual_artifact = actual.artifact();
    let artifact_fields = [
        (
            "kernel",
            expected_artifact.kernel_id(),
            actual_artifact.kernel_id(),
        ),
        (
            "kernel instance",
            expected_artifact.instance_digest(),
            actual_artifact.instance_digest(),
        ),
        (
            "source tree",
            expected_artifact.source_tree_digest(),
            actual_artifact.source_tree_digest(),
        ),
        (
            "crate graph",
            expected_artifact.crate_graph_digest(),
            actual_artifact.crate_graph_digest(),
        ),
        (
            "executable semantic",
            expected_artifact.executable_digest(),
            actual_artifact.executable_digest(),
        ),
        (
            "compiler and target environment",
            expected_artifact.environment_digest(),
            actual_artifact.environment_digest(),
        ),
        (
            "artifact selection",
            expected_artifact.artifact_selection_digest(),
            actual_artifact.artifact_selection_digest(),
        ),
        (
            "artifact contract",
            expected_artifact.artifact_contract_digest(),
            actual_artifact.artifact_contract_digest(),
        ),
    ];
    for (name, expected, actual) in artifact_fields {
        if expected != actual {
            return Err(ProofMatchError::IdentityMismatch(name));
        }
    }

    let expected_contracts = expected.source_contracts();
    let actual_contracts = actual.source_contracts();
    let source_contract_fields = [
        (
            "memory contract",
            expected_contracts.memory_digest(),
            actual_contracts.memory_digest(),
        ),
        (
            "effect contract",
            expected_contracts.effects_digest(),
            actual_contracts.effects_digest(),
        ),
        (
            "type layout contract",
            expected_contracts.type_layout_digest(),
            actual_contracts.type_layout_digest(),
        ),
        (
            "capability semantics contract",
            expected_contracts.capability_semantics_digest(),
            actual_contracts.capability_semantics_digest(),
        ),
        (
            "functional specification contract",
            expected_contracts.functional_specification_digest(),
            actual_contracts.functional_specification_digest(),
        ),
    ];
    for (name, expected, actual) in source_contract_fields {
        if expected != actual {
            return Err(ProofMatchError::IdentityMismatch(name));
        }
    }
    Ok(())
}

fn require_manifest_digest(
    field: &'static str,
    supplied: PayloadDigest,
    manifest: DigestBytes,
) -> Result<(), ProofTargetError> {
    if supplied.bytes() == manifest {
        Ok(())
    } else {
        Err(ProofTargetError::ManifestDigestMismatch(field))
    }
}

fn require_manifest_tool(
    field: &'static str,
    supplied: &MeasuredToolIdentity,
    manifest_name: &str,
    manifest_version: &str,
) -> Result<(), ProofTargetError> {
    if supplied.name().as_str() == manifest_name && supplied.version().as_str() == manifest_version
    {
        Ok(())
    } else {
        Err(ProofTargetError::ManifestToolMismatch(field))
    }
}

fn environment_bytes(
    manifest: &ManifestV1,
    compiler: &MeasuredToolIdentity,
    artifact_producer: &MeasuredToolIdentity,
) -> Vec<u8> {
    let mut writer = IdentityWriter::new(ARTIFACT_ENVIRONMENT_MAGIC);
    writer.measured_tool(compiler);
    writer.measured_tool(artifact_producer);
    writer.text(manifest.target().triple().as_str());
    writer.text(manifest.target().architecture().as_str());
    writer.u8(pointer_width_tag(manifest.target().pointer_width()));
    writer.u8(endianness_tag(manifest.target().endianness()));
    writer.capabilities(manifest.target().capabilities());
    writer.bytes
}

fn selection_bytes(
    kernel: &KernelEntry,
    code_object: &crate::CodeObjectIdentity,
    code_object_digest: PayloadDigest,
) -> Vec<u8> {
    let mut writer = IdentityWriter::new(ARTIFACT_SELECTION_MAGIC);
    writer.name(kernel.symbol());
    writer.payload_digest(code_object_digest);
    writer.u8(code_object_format_tag(code_object.format()));
    writer.u64(code_object.byte_len());
    writer.bytes
}

fn contract_bytes(kernel: &KernelEntry) -> Vec<u8> {
    let mut writer = IdentityWriter::new(ARTIFACT_CONTRACT_MAGIC);
    writer.name(kernel.name());
    writer.capabilities(kernel.required_capabilities());
    writer.launch(kernel.launch());
    writer.abi(kernel.abi());
    writer.bytes
}

struct IdentityWriter {
    bytes: Vec<u8>,
}

impl IdentityWriter {
    fn new(magic: [u8; 8]) -> Self {
        let mut writer = Self {
            bytes: Vec::with_capacity(256),
        };
        writer.bytes(&magic);
        writer.u16(PROOF_IDENTITY_VERSION);
        writer.u16(0);
        writer
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }

    fn name(&mut self, value: &Name) {
        self.text(value.as_str());
    }

    fn measured_tool(&mut self, value: &MeasuredToolIdentity) {
        self.text(value.name().as_str());
        self.text(value.version().as_str());
        self.payload_digest(value.executable_digest());
        self.payload_digest(value.configuration_digest());
    }

    fn payload_digest(&mut self, value: PayloadDigest) {
        self.u8(digest_algorithm_tag(value.algorithm()));
        self.bytes(value.bytes().as_bytes());
    }

    fn capabilities(&mut self, capabilities: &[crate::Capability]) {
        self.u16(capabilities.len() as u16);
        for capability in capabilities {
            self.u16(capability_tag(*capability));
        }
    }

    fn dimensions(&mut self, dimensions: crate::Dimensions) {
        self.u32(dimensions.x());
        self.u32(dimensions.y());
        self.u32(dimensions.z());
    }

    fn launch(&mut self, launch: &LaunchContract) {
        self.u8(launch.rank());
        match launch.block_size() {
            BlockSize::Any => self.u8(0),
            BlockSize::Exact(dimensions) => {
                self.u8(1);
                self.dimensions(dimensions);
            }
            BlockSize::AtMost(dimensions) => {
                self.u8(2);
                self.dimensions(dimensions);
            }
        }
        self.dimensions(launch.max_grid());
        self.u32(launch.static_shared_memory_bytes());
        self.u32(launch.max_dynamic_shared_memory_bytes());
    }

    fn abi(&mut self, abi: &AbiLayout) {
        self.u8(pointer_width_tag(abi.pointer_width()));
        self.u64(abi.size());
        self.u32(abi.alignment());
        self.u16(abi.fields().len() as u16);
        for field in abi.fields() {
            self.name(field.name());
            self.u64(field.offset());
            self.u64(field.size());
            self.u32(field.alignment());
            match field.kind() {
                AbiKind::Scalar(scalar) => {
                    self.u8(0);
                    self.u8(scalar_tag(scalar));
                }
                AbiKind::Pointer {
                    pointee_size,
                    pointee_alignment,
                } => {
                    self.u8(1);
                    self.u64(pointee_size);
                    self.u32(pointee_alignment);
                }
                AbiKind::Slice {
                    element_size,
                    element_alignment,
                } => {
                    self.u8(2);
                    self.u64(element_size);
                    self.u32(element_alignment);
                }
            }
            self.u8(mutability_tag(field.mutability()));
            self.u8(access_tag(field.access()));
            self.u8(address_space_tag(field.address_space()));
        }
    }
}
