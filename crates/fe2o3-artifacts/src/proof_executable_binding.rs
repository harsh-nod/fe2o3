use std::fmt;

use crate::encode::{
    access_tag, address_space_tag, capability_tag, endianness_tag, mutability_tag,
    pointer_width_tag, scalar_tag,
};
use crate::proof_encode::digest_algorithm_tag;
use crate::{
    AbiKind, AbiLayout, AliasClass, ArgumentOwnership, BlockSize, CodeObjectFormat,
    DigestAlgorithm, DigestBytes, LaunchContract, ManifestV1, MatchedProofEvidenceV1,
    MeasuredToolIdentity, PayloadDigest, ProofExecutionIdentity, ProofOutcome, ProofRecordV1,
    ProofTargetError, ProofTargetIdentity, SourceContractIdentity, TargetIdentity,
    V1_REQUIRED_PROPERTIES,
};

/// Domain and schema version for proof-to-executable binding identities.
pub const PROOF_EXECUTABLE_BINDING_DOMAIN_V1: [u8; 8] = *b"FE2OPXB\0";
pub const PROOF_EXECUTABLE_BINDING_VERSION_V1: u16 = 1;
const PROOF_POLICY_IDENTITY_DOMAIN_V1: [u8; 8] = *b"FE2OPOL\0";

/// AMDGPU HSA code-object version admitted by a proof-to-executable binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutableCodeObjectVersionV1 {
    V4,
    V5,
    V6,
}

impl ExecutableCodeObjectVersionV1 {
    pub const fn number(self) -> u8 {
        match self {
            Self::V4 => 4,
            Self::V5 => 5,
            Self::V6 => 6,
        }
    }
}

/// Exact executable semantics associated with one matched proof record.
///
/// This retains the full target, ABI, and launch contract so their identities
/// cannot be accidentally collapsed into an interchangeable untyped digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofExecutableSemanticIdentityV1 {
    proof_target: ProofTargetIdentity,
    finalized_code_object_digest: PayloadDigest,
    target: TargetIdentity,
    code_object_version: ExecutableCodeObjectVersionV1,
    abi: AbiLayout,
    launch: LaunchContract,
}

impl ProofExecutableSemanticIdentityV1 {
    pub const fn proof_target(&self) -> ProofTargetIdentity {
        self.proof_target
    }

    pub const fn kernel_semantic_identity(&self) -> PayloadDigest {
        self.proof_target.artifact().executable_digest()
    }

    pub const fn source_contracts(&self) -> SourceContractIdentity {
        self.proof_target.source_contracts()
    }

    pub const fn finalized_code_object_digest(&self) -> PayloadDigest {
        self.finalized_code_object_digest
    }

    pub const fn target(&self) -> &TargetIdentity {
        &self.target
    }

    pub const fn code_object_version(&self) -> ExecutableCodeObjectVersionV1 {
        self.code_object_version
    }

    pub const fn abi(&self) -> &AbiLayout {
        &self.abi
    }

    pub const fn launch(&self) -> &LaunchContract {
        &self.launch
    }
}

/// Complete measured tool closure and proof policy committed by a binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofToolPolicyIdentityV1 {
    compiler: MeasuredToolIdentity,
    artifact_producer: MeasuredToolIdentity,
    proof_execution: ProofExecutionIdentity,
    proof_policy_identity: PayloadDigest,
}

impl ProofToolPolicyIdentityV1 {
    pub const fn compiler(&self) -> &MeasuredToolIdentity {
        &self.compiler
    }

    pub const fn artifact_producer(&self) -> &MeasuredToolIdentity {
        &self.artifact_producer
    }

    pub const fn proof_execution(&self) -> &ProofExecutionIdentity {
        &self.proof_execution
    }

    pub const fn proof_policy_identity(&self) -> PayloadDigest {
        self.proof_policy_identity
    }
}

/// Inert evidence that one policy-matched proof record was associated with one
/// exact finalized executable identity.
///
/// Construction re-derives the complete proof target from the final manifest.
/// The binding is descriptive evidence only: it does not authenticate the
/// measurements, promote assurance, inspect bytes, or grant load/launch
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofExecutableBindingV1 {
    proof_record_digest: PayloadDigest,
    executable: ProofExecutableSemanticIdentityV1,
    tool_policy: ProofToolPolicyIdentityV1,
    binding_identity: PayloadDigest,
}

impl ProofExecutableBindingV1 {
    pub const fn version(&self) -> u16 {
        PROOF_EXECUTABLE_BINDING_VERSION_V1
    }

    pub const fn proof_record_digest(&self) -> PayloadDigest {
        self.proof_record_digest
    }

    pub const fn executable(&self) -> &ProofExecutableSemanticIdentityV1 {
        &self.executable
    }

    pub const fn tool_policy(&self) -> &ProofToolPolicyIdentityV1 {
        &self.tool_policy
    }

    pub const fn binding_identity(&self) -> PayloadDigest {
        self.binding_identity
    }

    /// Reconciles two independently constructed bindings field by field.
    pub fn validate_against(&self, actual: &Self) -> Result<(), ProofExecutableBindingError> {
        let expected_executable = &self.executable;
        let actual_executable = &actual.executable;
        require_equal(
            expected_executable.finalized_code_object_digest,
            actual_executable.finalized_code_object_digest,
            "finalized code-object digest",
        )?;
        require_equal(
            &expected_executable.target,
            &actual_executable.target,
            "target",
        )?;
        require_equal(
            expected_executable.code_object_version,
            actual_executable.code_object_version,
            "code-object version",
        )?;
        require_equal(&expected_executable.abi, &actual_executable.abi, "ABI")?;
        require_equal(
            &expected_executable.launch,
            &actual_executable.launch,
            "launch contract",
        )?;

        require_equal(
            &self.tool_policy.compiler,
            &actual.tool_policy.compiler,
            "compiler identity",
        )?;
        require_equal(
            &self.tool_policy.artifact_producer,
            &actual.tool_policy.artifact_producer,
            "artifact-producer identity",
        )?;
        validate_proof_target(
            expected_executable.proof_target,
            actual_executable.proof_target,
        )?;
        require_equal(
            &self.tool_policy.proof_execution,
            &actual.tool_policy.proof_execution,
            "proof-tool identity",
        )?;
        require_equal(
            self.tool_policy.proof_policy_identity,
            actual.tool_policy.proof_policy_identity,
            "proof-policy identity",
        )?;
        require_equal(
            self.proof_record_digest,
            actual.proof_record_digest,
            "proof-record digest",
        )?;
        require_equal(
            self.binding_identity,
            actual.binding_identity,
            "binding identity",
        )
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl MatchedProofEvidenceV1 {
    /// Binds this already policy-matched proof to a finalized native manifest
    /// entry and exact AMDGPU code-object version.
    ///
    /// The caller must supply algorithm-tagged measurements for the finalized
    /// code object, compiler, and artifact producer. This method assigns no
    /// digest algorithm to opaque manifest bytes.
    pub fn bind_finalized_executable_v1(
        &self,
        manifest: &ManifestV1,
        finalized_code_object_digest: PayloadDigest,
        code_object_version: ExecutableCodeObjectVersionV1,
        compiler: &MeasuredToolIdentity,
        artifact_producer: &MeasuredToolIdentity,
        binding_digest_algorithm: DigestAlgorithm,
    ) -> Result<ProofExecutableBindingV1, ProofExecutableBindingError> {
        let record = self.record();
        let target = record.target();
        let artifact = target.artifact();
        let kernel = manifest
            .kernels()
            .binary_search_by_key(&artifact.kernel_id().bytes(), |entry| entry.kernel_id())
            .ok()
            .map(|index| &manifest.kernels()[index])
            .ok_or(ProofExecutableBindingError::UnknownKernel(
                artifact.kernel_id().bytes(),
            ))?;

        let code_object = manifest
            .code_objects()
            .binary_search_by_key(&kernel.code_object_digest(), |object| object.digest())
            .ok()
            .map(|index| &manifest.code_objects()[index])
            .ok_or(ProofExecutableBindingError::MissingCodeObject(
                kernel.code_object_digest(),
            ))?;
        if code_object.format() != CodeObjectFormat::NativeExecutable {
            return Err(ProofExecutableBindingError::NonNativeCodeObject(
                code_object.format(),
            ));
        }

        let derived_algorithm = artifact.environment_digest().algorithm();
        if artifact.artifact_selection_digest().algorithm() != derived_algorithm
            || artifact.artifact_contract_digest().algorithm() != derived_algorithm
        {
            return Err(ProofExecutableBindingError::ProofTargetMismatch);
        }
        let reconstructed = manifest.proof_target(
            artifact.kernel_id(),
            artifact.instance_digest(),
            artifact.source_tree_digest(),
            artifact.crate_graph_digest(),
            artifact.executable_digest(),
            finalized_code_object_digest,
            target.source_contracts(),
            compiler,
            artifact_producer,
            derived_algorithm,
        )?;
        if reconstructed != target {
            return Err(ProofExecutableBindingError::ProofTargetMismatch);
        }

        let recalculated_record_digest = record.digest(self.record_digest().algorithm());
        if recalculated_record_digest != self.record_digest() {
            return Err(ProofExecutableBindingError::ProofRecordDigestMismatch);
        }

        let executable = ProofExecutableSemanticIdentityV1 {
            proof_target: target,
            finalized_code_object_digest,
            target: manifest.target().clone(),
            code_object_version,
            abi: kernel.abi().clone(),
            launch: kernel.launch().clone(),
        };
        let tool_policy = ProofToolPolicyIdentityV1 {
            compiler: compiler.clone(),
            artifact_producer: artifact_producer.clone(),
            proof_execution: record.execution().clone(),
            proof_policy_identity: proof_policy_identity(record, binding_digest_algorithm),
        };
        let proof_record_digest = self.record_digest();
        let binding_identity = binding_digest_algorithm.calculate(&binding_identity_bytes(
            proof_record_digest,
            &executable,
            &tool_policy,
        ));
        Ok(ProofExecutableBindingV1 {
            proof_record_digest,
            executable,
            tool_policy,
            binding_identity,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofExecutableBindingError {
    UnknownKernel(DigestBytes),
    MissingCodeObject(DigestBytes),
    NonNativeCodeObject(CodeObjectFormat),
    ProofTarget(ProofTargetError),
    ProofTargetMismatch,
    ProofRecordDigestMismatch,
    IdentityMismatch(&'static str),
}

impl fmt::Display for ProofExecutableBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKernel(_) => formatter.write_str("proof names an unknown kernel"),
            Self::MissingCodeObject(_) => {
                formatter.write_str("kernel names a missing finalized code object")
            }
            Self::NonNativeCodeObject(format) => {
                write!(
                    formatter,
                    "proof binding requires a native executable, found {format:?}"
                )
            }
            Self::ProofTarget(error) => {
                write!(formatter, "cannot reconstruct proof target: {error}")
            }
            Self::ProofTargetMismatch => {
                formatter.write_str("reconstructed executable proof target does not match")
            }
            Self::ProofRecordDigestMismatch => {
                formatter.write_str("matched proof record digest does not verify")
            }
            Self::IdentityMismatch(field) => write!(formatter, "{field} does not match"),
        }
    }
}

impl std::error::Error for ProofExecutableBindingError {}

impl From<ProofTargetError> for ProofExecutableBindingError {
    fn from(value: ProofTargetError) -> Self {
        Self::ProofTarget(value)
    }
}

fn require_equal<T: PartialEq>(
    expected: T,
    actual: T,
    field: &'static str,
) -> Result<(), ProofExecutableBindingError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ProofExecutableBindingError::IdentityMismatch(field))
    }
}

fn validate_proof_target(
    expected: ProofTargetIdentity,
    actual: ProofTargetIdentity,
) -> Result<(), ProofExecutableBindingError> {
    let expected_artifact = expected.artifact();
    let actual_artifact = actual.artifact();
    for (field, expected, actual) in [
        (
            "kernel identity",
            expected_artifact.kernel_id(),
            actual_artifact.kernel_id(),
        ),
        (
            "kernel instance identity",
            expected_artifact.instance_digest(),
            actual_artifact.instance_digest(),
        ),
        (
            "source-tree identity",
            expected_artifact.source_tree_digest(),
            actual_artifact.source_tree_digest(),
        ),
        (
            "crate-graph identity",
            expected_artifact.crate_graph_digest(),
            actual_artifact.crate_graph_digest(),
        ),
        (
            "kernel semantic identity",
            expected_artifact.executable_digest(),
            actual_artifact.executable_digest(),
        ),
        (
            "compiler/source environment identity",
            expected_artifact.environment_digest(),
            actual_artifact.environment_digest(),
        ),
        (
            "artifact-selection identity",
            expected_artifact.artifact_selection_digest(),
            actual_artifact.artifact_selection_digest(),
        ),
        (
            "artifact-contract identity",
            expected_artifact.artifact_contract_digest(),
            actual_artifact.artifact_contract_digest(),
        ),
    ] {
        require_equal(expected, actual, field)?;
    }

    let expected_contracts = expected.source_contracts();
    let actual_contracts = actual.source_contracts();
    for (field, expected, actual) in [
        (
            "memory-contract identity",
            expected_contracts.memory_digest(),
            actual_contracts.memory_digest(),
        ),
        (
            "effects-contract identity",
            expected_contracts.effects_digest(),
            actual_contracts.effects_digest(),
        ),
        (
            "type-layout identity",
            expected_contracts.type_layout_digest(),
            actual_contracts.type_layout_digest(),
        ),
        (
            "capability-semantics identity",
            expected_contracts.capability_semantics_digest(),
            actual_contracts.capability_semantics_digest(),
        ),
        (
            "functional-specification identity",
            expected_contracts.functional_specification_digest(),
            actual_contracts.functional_specification_digest(),
        ),
    ] {
        require_equal(expected, actual, field)?;
    }
    Ok(())
}

fn proof_policy_identity(record: &ProofRecordV1, algorithm: DigestAlgorithm) -> PayloadDigest {
    let canonical_policy = ProofRecordV1::new(
        record.target(),
        record.configuration().to_vec(),
        record.execution().clone(),
        ProofOutcome::Proved,
        V1_REQUIRED_PROPERTIES.to_vec(),
        record.trusted_items().to_vec(),
    )
    .expect("a validated matched record always yields a valid canonical policy record");
    let mut bytes = Vec::with_capacity(canonical_policy.to_bytes().len() + 12);
    bytes.extend_from_slice(&PROOF_POLICY_IDENTITY_DOMAIN_V1);
    bytes.extend_from_slice(&PROOF_EXECUTABLE_BINDING_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&canonical_policy.to_bytes());
    algorithm.calculate(&bytes)
}

fn binding_identity_bytes(
    proof_record_digest: PayloadDigest,
    executable: &ProofExecutableSemanticIdentityV1,
    tool_policy: &ProofToolPolicyIdentityV1,
) -> Vec<u8> {
    let mut writer = BindingIdentityWriter::new();
    writer.payload_digest(proof_record_digest);
    writer.proof_target(executable.proof_target);
    writer.payload_digest(executable.finalized_code_object_digest);
    writer.target(&executable.target);
    writer.u8(executable.code_object_version.number());
    writer.abi(&executable.abi);
    writer.launch(&executable.launch);
    writer.measured_tool(&tool_policy.compiler);
    writer.measured_tool(&tool_policy.artifact_producer);
    writer.proof_execution(&tool_policy.proof_execution);
    writer.payload_digest(tool_policy.proof_policy_identity);
    writer.bytes
}

struct BindingIdentityWriter {
    bytes: Vec<u8>,
}

impl BindingIdentityWriter {
    fn new() -> Self {
        let mut writer = Self {
            bytes: Vec::with_capacity(1024),
        };
        writer.bytes(&PROOF_EXECUTABLE_BINDING_DOMAIN_V1);
        writer.u16(PROOF_EXECUTABLE_BINDING_VERSION_V1);
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

    fn payload_digest(&mut self, value: PayloadDigest) {
        self.u8(digest_algorithm_tag(value.algorithm()));
        self.bytes(value.bytes().as_bytes());
    }

    fn measured_tool(&mut self, value: &MeasuredToolIdentity) {
        self.text(value.name().as_str());
        self.text(value.version().as_str());
        self.payload_digest(value.executable_digest());
        self.payload_digest(value.configuration_digest());
    }

    fn proof_execution(&mut self, value: &ProofExecutionIdentity) {
        self.text(value.model().version().as_str());
        self.payload_digest(value.model().axioms_digest());
        self.measured_tool(value.verifier());
        self.measured_tool(value.solver());
        self.measured_tool(value.evidence_recorder());
        self.payload_digest(value.invocation_digest());
    }

    fn proof_target(&mut self, value: ProofTargetIdentity) {
        let artifact = value.artifact();
        for digest in [
            artifact.kernel_id(),
            artifact.instance_digest(),
            artifact.source_tree_digest(),
            artifact.crate_graph_digest(),
            artifact.executable_digest(),
            artifact.environment_digest(),
            artifact.artifact_selection_digest(),
            artifact.artifact_contract_digest(),
        ] {
            self.payload_digest(digest);
        }
        let contracts = value.source_contracts();
        for digest in [
            contracts.memory_digest(),
            contracts.effects_digest(),
            contracts.type_layout_digest(),
            contracts.capability_semantics_digest(),
            contracts.functional_specification_digest(),
        ] {
            self.payload_digest(digest);
        }
    }

    fn target(&mut self, value: &TargetIdentity) {
        self.text(value.triple().as_str());
        self.text(value.architecture().as_str());
        self.u8(pointer_width_tag(value.pointer_width()));
        self.u8(endianness_tag(value.endianness()));
        self.u16(value.capabilities().len() as u16);
        for capability in value.capabilities() {
            self.u16(capability_tag(*capability));
        }
    }

    fn dimensions(&mut self, value: crate::Dimensions) {
        self.u32(value.x());
        self.u32(value.y());
        self.u32(value.z());
    }

    fn launch(&mut self, value: &LaunchContract) {
        self.u8(value.rank());
        match value.block_size() {
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
        self.dimensions(value.max_grid());
        self.u32(value.static_shared_memory_bytes());
        self.u32(value.max_dynamic_shared_memory_bytes());
    }

    fn abi(&mut self, value: &AbiLayout) {
        self.u8(pointer_width_tag(value.pointer_width()));
        self.u64(value.size());
        self.u32(value.alignment());
        self.u16(value.fields().len() as u16);
        for field in value.fields() {
            self.text(field.name().as_str());
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
            self.bytes(field.type_identity().rust_type().bytes().as_bytes());
            self.bytes(field.type_identity().layout().bytes().as_bytes());
            self.u8(ownership_tag(field.ownership()));
            self.u8(alias_class_tag(field.alias_class()));
        }
    }
}

const fn ownership_tag(value: ArgumentOwnership) -> u8 {
    match value {
        ArgumentOwnership::ByValue => 0,
        ArgumentOwnership::SharedBorrow => 1,
        ArgumentOwnership::UniqueBorrow => 2,
        ArgumentOwnership::RawPointer => 3,
    }
}

const fn alias_class_tag(value: AliasClass) -> u8 {
    match value {
        AliasClass::Value => 0,
        AliasClass::SharedReadOnly => 1,
        AliasClass::Exclusive => 2,
        AliasClass::SharedAtomic => 3,
        AliasClass::Unrestricted => 4,
    }
}
