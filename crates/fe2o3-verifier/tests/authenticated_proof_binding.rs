use std::fs;
use std::sync::{Mutex, OnceLock};

use fe2o3_artifacts::{
    AbiLayout, BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity, CompilerIdentity,
    ConfigurationEntry as ArtifactConfigurationEntry, DigestAlgorithm, DigestBytes, Dimensions,
    Endianness, ExecutableCodeObjectVersionV1, IdentityText, KernelEntry, LaunchContract,
    ManifestV1, MeasuredToolIdentity as ArtifactMeasuredToolIdentity, Name, PayloadDigest,
    PointerWidth, ProofExecutionIdentity, ProofMatchPolicy, ProofTargetIdentity as ArtifactTarget,
    SourceContractIdentity, TargetIdentity, ToolIdentity, TrustedItem as ArtifactTrustedItem,
    V1_REQUIRED_PROPERTIES, VerificationModelIdentity as ArtifactModel,
};
use fe2o3_contracts::{
    AddressSpaceIdV1, AllocationProvenanceIdV1, AllocationSpecV1, ByteRegionV1,
    StaticViewAccessDescriptionV1, StaticViewDescriptionV1,
};
use fe2o3_rustc_front::{
    ControlFlowContractV1, ControlFlowNodeIdV1, ControlFlowNodeKindV1, ControlFlowNodeV1,
    FrontendIntegerSwitchCaseV1, FrontendIntegerSwitchTypeV1, FrontendSourceSpanV1,
    encode_control_flow_contract_v1,
};
use fe2o3_verifier::{
    AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
    AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
    AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1, AlphaZetaExecutableEvidenceReviewV1,
    AlphaZetaProofErrorV1, AlphaZetaProofSourcesV1, AuthenticatedControlFlowExecutableBindingV1,
    AuthenticatedExecutionFreshnessV1, AuthenticatedProofExecutableBindingError,
    AuthenticatedProofExecutablePolicyV1, AuthenticatedRecorderOutputV1, AxiomPolicy,
    Configuration, ConfigurationEntry, ControlFlowBindingErrorV1, ControlFlowClaimsV1,
    ControlFlowIntegerSwitchCaseClaimV1, ControlFlowIntegerSwitchClaimV1, ControlFlowLoopClaimV1,
    CorrelationId, Digest, ExecutionLimits, ExecutionTools,
    GFX942_ALPHA_ZETA_AUTHENTICATED_PROPERTIES_V1, GFX942_ALPHA_ZETA_MODEL_VERSION_V1,
    Gfx942AlphaZetaKernelV1, Gfx942AlphaZetaProofInputV1, Gfx942XnackMinusTargetIdentityV1,
    InertAlphaZetaExecutableEvidenceSetV1, KernelProofAdmissionIdentityV1,
    KernelProofAdmissionRequestV1, MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1,
    MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1, MeasuredRecorderInputsV1, MeasuredToolIdentity,
    MultiKernelProofAdmissionErrorV1, MultiKernelProofAdmissionV1,
    PERSISTENT_AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
    PERSISTENT_AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
    PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1,
    PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1, PersistentFreshnessIdentityFieldV1,
    PersistentFreshnessLedgerErrorV1, PersistentFreshnessRecoveryV1,
    PersistentProofFreshnessLedgerV1, PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
    PersistentlyFreshKernelProofAdmissionIdentityV1,
    PersistentlyFreshKernelProofAdmissionRequestV1,
    PersistentlyFreshMultiKernelProofAdmissionErrorV1,
    PersistentlyFreshMultiKernelProofAdmissionV1, ProcessLocalProofCapsuleDuplicateDetectorV1,
    ProofCapsuleBuildErrorV1, ProofCapsuleContextErrorV1, ProofCapsuleExpectationV1,
    ProofCapsuleFreshnessExpectationV1, ProofCapsuleFreshnessIdentityV1,
    ProofCapsuleIdentityFieldV1, ProofCapsuleTargetV1, ProofCapsuleV1, ProofOutcome, ProofProperty,
    ProofRequestV1, ProofTargetIdentity, STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1,
    StaticViewLifetimeEpochClaimV1, StaticViewProofObligationV1, VerificationModelIdentity,
    VerifierPolicy, alpha_zeta_abi_identity_v1, alpha_zeta_inert_configuration_v1,
    alpha_zeta_launch_identity_v1, bind_authenticated_control_flow_executable_v1,
    bind_authenticated_proof_executable_persistent_v1, bind_authenticated_proof_executable_v1,
    bind_control_flow_proof_request_v1,
    bind_persistently_fresh_authenticated_control_flow_executable_v1,
    bind_static_view_proof_evidence_v1, derive_control_flow_functional_specification_digest_v1,
    derive_static_view_functional_specification_digest_v1, execute_authenticated_recorder,
    reconcile_control_flow_source_v1, record_inert_alpha_zeta_executable_evidence_v1,
};

static SYNTHETIC_RECORDER_EXECUTION: Mutex<()> = Mutex::new(());

const ALL_PROPERTIES: [ProofProperty; 7] = [
    ProofProperty::Bounds,
    ProofProperty::AddressOverflowFreedom,
    ProofProperty::MemorySafety,
    ProofProperty::Initialization,
    ProofProperty::RaceFreedom,
    ProofProperty::LaunchValidity,
    ProofProperty::FunctionalCorrectness,
];

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn bytes(seed: u8) -> DigestBytes {
    DigestBytes::from_bytes([seed; 32])
}

fn payload(seed: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, bytes(seed))
}

fn payload_from_digest(value: Digest) -> PayloadDigest {
    PayloadDigest::new(
        DigestAlgorithm::Sha256,
        DigestBytes::from_bytes(*value.as_bytes()),
    )
}

fn verifier_digest(value: PayloadDigest) -> Digest {
    Digest::from_bytes(*value.bytes().as_bytes())
}

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn synthetic_recorder_fixture() -> &'static str {
    option_env!("CARGO_BIN_EXE_fe2o3-verifier-test-recorder")
        .expect("Cargo did not provide the verifier test recorder")
}

#[cfg(target_os = "linux")]
struct PersistentLedgerDirectory {
    path: std::path::PathBuf,
}

#[cfg(target_os = "linux")]
impl PersistentLedgerDirectory {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
        let nonce = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-authenticated-binding-ledger-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self { path }
    }
}

#[cfg(target_os = "linux")]
impl Drop for PersistentLedgerDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn artifact_tool(name: &str, version: &str, executable: u8) -> ArtifactMeasuredToolIdentity {
    ArtifactMeasuredToolIdentity::new(
        text(name),
        text(version),
        payload(executable),
        payload(executable + 1),
    )
}

fn compiler() -> ArtifactMeasuredToolIdentity {
    artifact_tool("rustc", "1.94.0", 0x60)
}

fn producer() -> ArtifactMeasuredToolIdentity {
    artifact_tool("fe2o3", "0.1.0", 0x62)
}

fn source_contracts() -> SourceContractIdentity {
    source_contracts_with_functional(payload(0x54))
}

fn source_contracts_with_functional(
    functional_specification_digest: PayloadDigest,
) -> SourceContractIdentity {
    SourceContractIdentity::new(
        payload(0x50),
        payload(0x51),
        payload(0x52),
        payload(0x53),
        functional_specification_digest,
    )
}

fn manifest() -> ManifestV1 {
    manifest_for_processor("gfx942")
}

fn manifest_for_processor(processor: &str) -> ManifestV1 {
    manifest_for_bundle(processor, 0x44)
}

fn manifest_for_bundle(processor: &str, code_object_digest: u8) -> ManifestV1 {
    let target = TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text(processor),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::AmdWave],
    )
    .unwrap();
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(u32::MAX, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    let abi = AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap();
    let object = CodeObjectIdentity::new(
        bytes(code_object_digest),
        CodeObjectFormat::NativeExecutable,
        4096,
    )
    .unwrap();
    let first_kernel = KernelEntry::new(
        bytes(0x11),
        name("verified_kernel"),
        name("verified_kernel.kd"),
        bytes(0x22),
        bytes(0x33),
        object.digest(),
        vec![Capability::AmdWave],
        launch.clone(),
        abi.clone(),
    )
    .unwrap();
    let second_kernel = KernelEntry::new(
        bytes(0x12),
        name("verified_kernel_second"),
        name("verified_kernel_second.kd"),
        bytes(0x23),
        bytes(0x34),
        object.digest(),
        vec![Capability::AmdWave],
        launch,
        abi,
    )
    .unwrap();
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target,
        vec![object],
        vec![first_kernel, second_kernel],
    )
    .unwrap()
}

fn artifact_target(manifest: &ManifestV1) -> ArtifactTarget {
    artifact_target_with_contracts(manifest, source_contracts())
}

fn artifact_target_with_contracts(
    manifest: &ManifestV1,
    source_contracts: SourceContractIdentity,
) -> ArtifactTarget {
    artifact_target_for_kernel(manifest, 0x11, 0x22, 0x33, source_contracts)
}

fn artifact_target_for_kernel(
    manifest: &ManifestV1,
    kernel_id: u8,
    source: u8,
    executable: u8,
    source_contracts: SourceContractIdentity,
) -> ArtifactTarget {
    artifact_target_for_kernel_with_bundle(
        manifest,
        kernel_id,
        source,
        executable,
        source_contracts,
        payload(0x44),
        &compiler(),
        &producer(),
    )
}

#[allow(clippy::too_many_arguments)]
fn artifact_target_for_kernel_with_bundle(
    manifest: &ManifestV1,
    kernel_id: u8,
    source: u8,
    executable: u8,
    source_contracts: SourceContractIdentity,
    finalized_executable_digest: PayloadDigest,
    compiler: &ArtifactMeasuredToolIdentity,
    producer: &ArtifactMeasuredToolIdentity,
) -> ArtifactTarget {
    manifest
        .proof_target(
            payload(kernel_id),
            payload(0x40),
            payload(source),
            payload(0x41),
            payload(executable),
            finalized_executable_digest,
            source_contracts,
            compiler,
            producer,
            DigestAlgorithm::Sha256,
        )
        .unwrap()
}

fn control_flow_source() -> (Vec<u8>, Vec<u8>, ControlFlowClaimsV1) {
    control_flow_source_with("src/kernel.rs", 8)
}

fn control_flow_source_with(
    source_path: &str,
    max_iterations: u32,
) -> (Vec<u8>, Vec<u8>, ControlFlowClaimsV1) {
    let id = ControlFlowNodeIdV1::new;
    let span = |line| FrontendSourceSpanV1::new(source_path, line, 1, line, 8).unwrap();
    let switch = ControlFlowNodeKindV1::integer_switch(
        FrontendIntegerSwitchTypeV1::new(32, false).unwrap(),
        vec![FrontendIntegerSwitchCaseV1::from_unsigned(0, id(3))],
        id(4),
    )
    .unwrap();
    let contract = ControlFlowContractV1::new(
        id(0),
        vec![
            ControlFlowNodeV1::new(
                id(0),
                span(10),
                ControlFlowNodeKindV1::Entry { target: id(1) },
            ),
            ControlFlowNodeV1::new(
                id(1),
                span(11),
                ControlFlowNodeKindV1::Loop {
                    max_iterations,
                    body: id(2),
                    exit: id(5),
                },
            ),
            ControlFlowNodeV1::new(id(2), span(12), switch),
            ControlFlowNodeV1::new(
                id(3),
                span(13),
                ControlFlowNodeKindV1::Continue {
                    loop_header: id(1),
                    target: id(1),
                },
            ),
            ControlFlowNodeV1::new(
                id(4),
                span(14),
                ControlFlowNodeKindV1::Break {
                    loop_header: id(1),
                    target: id(5),
                },
            ),
            ControlFlowNodeV1::new(id(5), span(15), ControlFlowNodeKindV1::Exit),
        ],
    )
    .unwrap();
    let claims = ControlFlowClaimsV1::new(
        vec![ControlFlowLoopClaimV1::new(1, max_iterations).unwrap()],
        vec![
            ControlFlowIntegerSwitchClaimV1::new(
                2,
                32,
                false,
                vec![ControlFlowIntegerSwitchCaseClaimV1::new(0, 3)],
                4,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    (
        encode_control_flow_contract_v1(&contract).unwrap(),
        contract.cfg_identity().as_bytes().to_vec(),
        claims,
    )
}

fn verifier_target(target: ArtifactTarget) -> ProofTargetIdentity {
    let artifact = target.artifact();
    let contracts = target.source_contracts();
    ProofTargetIdentity {
        kernel_id: verifier_digest(artifact.kernel_id()),
        instance_digest: verifier_digest(artifact.instance_digest()),
        source_tree_digest: verifier_digest(artifact.source_tree_digest()),
        crate_graph_digest: verifier_digest(artifact.crate_graph_digest()),
        executable_digest: verifier_digest(artifact.executable_digest()),
        environment_digest: verifier_digest(artifact.environment_digest()),
        artifact_selection_digest: verifier_digest(artifact.artifact_selection_digest()),
        artifact_contract_digest: verifier_digest(artifact.artifact_contract_digest()),
        memory_contract_digest: verifier_digest(contracts.memory_digest()),
        effects_contract_digest: verifier_digest(contracts.effects_digest()),
        type_layout_digest: verifier_digest(contracts.type_layout_digest()),
        capability_semantics_digest: verifier_digest(contracts.capability_semantics_digest()),
        functional_specification_digest: verifier_digest(
            contracts.functional_specification_digest(),
        ),
    }
}

fn proof_capsule_target(
    target: ProofTargetIdentity,
    finalized_payload_identity: Digest,
) -> ProofCapsuleTargetV1 {
    ProofCapsuleTargetV1::new(
        target,
        vec![],
        vec![],
        digest(0xa0),
        digest(0xa1),
        digest(0xa2),
        finalized_payload_identity,
        digest(0xa3),
    )
    .unwrap()
}

fn configuration() -> Configuration {
    Configuration::new(vec![ConfigurationEntry::new("solver", "z3").unwrap()]).unwrap()
}

fn model() -> VerificationModelIdentity {
    VerificationModelIdentity::new("gpu-model-v1", digest(0x70)).unwrap()
}

fn measured_tool(name: &str, executable_digest: Digest, config: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(name, "test-v1", executable_digest, digest(config)).unwrap()
}

fn synthetic_claimed_tools() -> ExecutionTools {
    synthetic_claimed_tools_with_verifier_configuration(0x71)
}

fn synthetic_claimed_tools_with_verifier_configuration(
    verifier_configuration: u8,
) -> ExecutionTools {
    let executable_digest = sha256(&fs::read(synthetic_recorder_fixture()).unwrap());
    ExecutionTools::new(
        measured_tool("claimed-verus", executable_digest, verifier_configuration),
        measured_tool("claimed-z3", executable_digest, 0x72),
        measured_tool("synthetic-recorder", executable_digest, 0x73),
    )
}

fn make_verifier_policy(tools: ExecutionTools, max_timeout_seconds: u32) -> VerifierPolicy {
    VerifierPolicy::new(
        tools,
        configuration(),
        model(),
        AxiomPolicy::deny_all(),
        max_timeout_seconds,
    )
    .unwrap()
}

fn synthetic_recorder_output(
    target: ArtifactTarget,
) -> (AuthenticatedRecorderOutputV1, VerifierPolicy) {
    synthetic_recorder_output_with_tools(target, synthetic_claimed_tools())
}

fn synthetic_recorder_output_with_tools(
    target: ArtifactTarget,
    tools: ExecutionTools,
) -> (AuthenticatedRecorderOutputV1, VerifierPolicy) {
    synthetic_recorder_output_with_tools_and_limits(target, tools, 10, 10)
}

fn synthetic_recorder_output_with_tools_and_limits(
    target: ArtifactTarget,
    tools: ExecutionTools,
    policy_timeout_seconds: u32,
    invocation_timeout_seconds: u32,
) -> (AuthenticatedRecorderOutputV1, VerifierPolicy) {
    // The synthetic debug recorder is roughly 17 MiB and deliberately
    // self-hashes before emitting a claimed result. It never executes the
    // supplied verifier or solver paths. Its wall-clock runtime is
    // scheduler-sensitive under concurrent test load, so fixtures serialize
    // that work and use the policy's existing 10-second ceiling. Focused
    // one-second timeout coverage remains in the executor tests.
    let _execution = SYNTHETIC_RECORDER_EXECUTION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let verifier_policy = make_verifier_policy(tools, policy_timeout_seconds);
    let request = ProofRequestV1::new(
        CorrelationId::from_bytes([51; 16]),
        verifier_target(target),
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let inputs = MeasuredRecorderInputsV1::new(
        synthetic_recorder_fixture(),
        synthetic_recorder_fixture(),
        synthetic_recorder_fixture(),
    )
    .unwrap();
    let evidence = execute_authenticated_recorder(
        request,
        inputs,
        invocation_timeout_seconds,
        &verifier_policy,
        ExecutionLimits::default(),
    )
    .unwrap();
    (evidence, verifier_policy)
}

fn artifact_execution(evidence: &AuthenticatedRecorderOutputV1) -> ProofExecutionIdentity {
    let tools = evidence.invocation_plan().tools();
    ProofExecutionIdentity::new(
        ArtifactModel::new(text(model().version().as_str()), payload(0x70)),
        artifact_execution_tool(tools.verifier()),
        artifact_execution_tool(tools.solver()),
        artifact_execution_tool(tools.evidence_recorder()),
        payload_from_digest(evidence.canonical_invocation_digest()),
    )
}

fn artifact_execution_tool(tool: &MeasuredToolIdentity) -> ArtifactMeasuredToolIdentity {
    ArtifactMeasuredToolIdentity::new(
        text(tool.name().as_str()),
        text(tool.version().as_str()),
        payload_from_digest(tool.executable_digest()),
        payload_from_digest(tool.configuration_digest()),
    )
}

fn proof_policy(
    target: ArtifactTarget,
    evidence: &AuthenticatedRecorderOutputV1,
) -> ProofMatchPolicy {
    ProofMatchPolicy::new(
        target,
        vec![ArtifactConfigurationEntry::new(name("solver"), text("z3"))],
        artifact_execution(evidence),
        vec![],
    )
    .unwrap()
}

fn binding_policy(
    manifest: ManifestV1,
    target: ArtifactTarget,
    evidence: &AuthenticatedRecorderOutputV1,
    verifier_policy: VerifierPolicy,
) -> AuthenticatedProofExecutablePolicyV1 {
    binding_policy_with_bundle(
        manifest,
        target,
        evidence,
        verifier_policy,
        payload(0x44),
        ExecutableCodeObjectVersionV1::V6,
        compiler(),
        producer(),
    )
}

#[allow(clippy::too_many_arguments)]
fn binding_policy_with_bundle(
    manifest: ManifestV1,
    target: ArtifactTarget,
    evidence: &AuthenticatedRecorderOutputV1,
    verifier_policy: VerifierPolicy,
    finalized_executable_digest: PayloadDigest,
    code_object_version: ExecutableCodeObjectVersionV1,
    compiler: ArtifactMeasuredToolIdentity,
    producer: ArtifactMeasuredToolIdentity,
) -> AuthenticatedProofExecutablePolicyV1 {
    AuthenticatedProofExecutablePolicyV1::new(
        verifier_policy,
        proof_policy(target, evidence),
        manifest,
        finalized_executable_digest,
        code_object_version,
        compiler,
        producer,
        DigestAlgorithm::Sha256,
    )
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
enum AlphaZetaConfigurationMutation {
    Exact,
    ProofNonce,
    Extra,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
enum AlphaZetaIdentityMutation {
    Exact,
    Abi,
    Launch,
}

#[cfg(target_os = "linux")]
fn alpha_zeta_sources() -> AlphaZetaProofSourcesV1 {
    AlphaZetaProofSourcesV1::discover_workspace(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
fn alpha_zeta_manifest(sources: &AlphaZetaProofSourcesV1, processor: &str) -> ManifestV1 {
    let target = TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text(processor),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::AmdWave],
    )
    .unwrap();
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(u32::MAX, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    let abi = AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap();
    let object =
        CodeObjectIdentity::new(bytes(0x44), CodeObjectFormat::NativeExecutable, 4096).unwrap();
    let source = DigestBytes::from_bytes(*sources.source_tree_identity().as_bytes());
    let alpha = KernelEntry::new(
        bytes(0xa1),
        name("alpha"),
        name("alpha.kd"),
        source,
        bytes(0xb1),
        object.digest(),
        vec![Capability::AmdWave],
        launch.clone(),
        abi.clone(),
    )
    .unwrap();
    let zeta = KernelEntry::new(
        bytes(0xa2),
        name("zeta"),
        name("zeta.kd"),
        source,
        bytes(0xb2),
        object.digest(),
        vec![Capability::AmdWave],
        launch,
        abi,
    )
    .unwrap();
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target,
        vec![object],
        vec![alpha, zeta],
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
fn alpha_zeta_artifact_target(
    manifest: &ManifestV1,
    sources: &AlphaZetaProofSourcesV1,
    kernel: Gfx942AlphaZetaKernelV1,
) -> ArtifactTarget {
    let (kernel_id, executable) = match kernel {
        Gfx942AlphaZetaKernelV1::Alpha => (payload(0xa1), payload(0xb1)),
        Gfx942AlphaZetaKernelV1::Zeta => (payload(0xa2), payload(0xb2)),
    };
    manifest
        .proof_target(
            kernel_id,
            payload(0x40),
            payload_from_digest(sources.source_tree_identity()),
            payload_from_digest(sources.dependency_tree_identity()),
            executable,
            payload(0x44),
            source_contracts(),
            &compiler(),
            &producer(),
            DigestAlgorithm::Sha256,
        )
        .unwrap()
}

#[cfg(target_os = "linux")]
fn alpha_zeta_tools() -> ExecutionTools {
    let executable_digest = sha256(&fs::read(synthetic_recorder_fixture()).unwrap());
    ExecutionTools::new(
        measured_tool("verus", executable_digest, 0xc1),
        measured_tool("z3", executable_digest, 0xc2),
        measured_tool("alpha-zeta-recorder", executable_digest, 0xc3),
    )
}

#[cfg(target_os = "linux")]
fn alpha_zeta_model() -> VerificationModelIdentity {
    VerificationModelIdentity::new(GFX942_ALPHA_ZETA_MODEL_VERSION_V1, digest(0xc4)).unwrap()
}

#[cfg(target_os = "linux")]
fn alpha_zeta_configuration(
    input: &Gfx942AlphaZetaProofInputV1,
    mutation: AlphaZetaConfigurationMutation,
) -> Configuration {
    let mut entries = alpha_zeta_inert_configuration_v1(input);
    if matches!(mutation, AlphaZetaConfigurationMutation::ProofNonce) {
        entries
            .iter_mut()
            .find(|(key, _)| *key == "proof_nonce")
            .unwrap()
            .1 = "00".repeat(32);
    }
    if matches!(mutation, AlphaZetaConfigurationMutation::Extra) {
        entries.push(("unbound_option", "present".to_owned()));
    }
    Configuration::new(
        entries
            .into_iter()
            .map(|(key, value)| ConfigurationEntry::new(key, value).unwrap())
            .collect(),
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
fn artifact_configuration(configuration: &Configuration) -> Vec<ArtifactConfigurationEntry> {
    configuration
        .entries()
        .iter()
        .map(|entry| {
            ArtifactConfigurationEntry::new(
                name(entry.key().as_str()),
                text(entry.value().as_str()),
            )
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn alpha_zeta_artifact_execution(
    evidence: &AuthenticatedRecorderOutputV1,
    model: &VerificationModelIdentity,
) -> ProofExecutionIdentity {
    let tools = evidence.invocation_plan().tools();
    ProofExecutionIdentity::new(
        ArtifactModel::new(
            text(model.version().as_str()),
            payload_from_digest(model.axioms_digest()),
        ),
        artifact_execution_tool(tools.verifier()),
        artifact_execution_tool(tools.solver()),
        artifact_execution_tool(tools.evidence_recorder()),
        payload_from_digest(evidence.canonical_invocation_digest()),
    )
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn alpha_zeta_persistent_binding(
    kernel: Gfx942AlphaZetaKernelV1,
    processor: &str,
    proof_set_nonce: Digest,
    proof_nonce: Digest,
    identity_mutation: AlphaZetaIdentityMutation,
    mutation: AlphaZetaConfigurationMutation,
    freshness: &mut PersistentProofFreshnessLedgerV1,
) -> (
    Gfx942AlphaZetaProofInputV1,
    fe2o3_verifier::PersistentlyFreshProofExecutableBindingV1,
) {
    alpha_zeta_persistent_binding_with_role_and_cov(
        kernel,
        kernel,
        processor,
        proof_set_nonce,
        proof_nonce,
        identity_mutation,
        mutation,
        ExecutableCodeObjectVersionV1::V6,
        freshness,
    )
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn alpha_zeta_persistent_binding_with_role_and_cov(
    executable_kernel: Gfx942AlphaZetaKernelV1,
    input_role: Gfx942AlphaZetaKernelV1,
    processor: &str,
    proof_set_nonce: Digest,
    proof_nonce: Digest,
    identity_mutation: AlphaZetaIdentityMutation,
    mutation: AlphaZetaConfigurationMutation,
    code_object_version: ExecutableCodeObjectVersionV1,
    freshness: &mut PersistentProofFreshnessLedgerV1,
) -> (
    Gfx942AlphaZetaProofInputV1,
    fe2o3_verifier::PersistentlyFreshProofExecutableBindingV1,
) {
    let sources = alpha_zeta_sources();
    let trusted_items = sources.trusted_inventory().trusted_items().to_vec();
    let manifest = alpha_zeta_manifest(&sources, processor);
    let artifact_target = alpha_zeta_artifact_target(&manifest, &sources, executable_kernel);
    let target = verifier_target(artifact_target);
    let tools = alpha_zeta_tools();
    let model = alpha_zeta_model();
    let entry = manifest
        .kernels()
        .iter()
        .find(|entry| {
            entry.kernel_id()
                == match executable_kernel {
                    Gfx942AlphaZetaKernelV1::Alpha => bytes(0xa1),
                    Gfx942AlphaZetaKernelV1::Zeta => bytes(0xa2),
                }
        })
        .unwrap();
    let mut abi_identity = alpha_zeta_abi_identity_v1(entry.abi());
    let mut launch_identity = alpha_zeta_launch_identity_v1(entry.launch());
    match identity_mutation {
        AlphaZetaIdentityMutation::Exact => {}
        AlphaZetaIdentityMutation::Abi => abi_identity = digest(0xf1),
        AlphaZetaIdentityMutation::Launch => launch_identity = digest(0xf2),
    }
    let input = Gfx942AlphaZetaProofInputV1::seal(
        input_role,
        sources,
        target,
        abi_identity,
        verifier_digest(source_contracts().effects_digest()),
        launch_identity,
        tools.verifier().clone(),
        tools.solver().clone(),
        model.clone(),
        proof_set_nonce,
        proof_nonce,
    )
    .unwrap();
    let configuration = alpha_zeta_configuration(&input, mutation);
    let verifier_policy = VerifierPolicy::new(
        tools,
        configuration.clone(),
        model.clone(),
        AxiomPolicy::allow_list(trusted_items.clone()).unwrap(),
        10,
    )
    .unwrap();
    let request = ProofRequestV1::new(
        CorrelationId::from_bytes([51; 16]),
        target,
        configuration.clone(),
        model.clone(),
        GFX942_ALPHA_ZETA_AUTHENTICATED_PROPERTIES_V1.to_vec(),
        trusted_items.clone(),
    )
    .unwrap();
    let inputs = MeasuredRecorderInputsV1::new(
        synthetic_recorder_fixture(),
        synthetic_recorder_fixture(),
        synthetic_recorder_fixture(),
    )
    .unwrap();
    let evidence = {
        let _execution = SYNTHETIC_RECORDER_EXECUTION
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        execute_authenticated_recorder(
            request,
            inputs,
            10,
            &verifier_policy,
            ExecutionLimits::default(),
        )
        .unwrap()
    };
    let artifact_trusted_items = trusted_items
        .iter()
        .map(|item| {
            ArtifactTrustedItem::new(
                name(item.name().as_str()),
                payload_from_digest(item.contract_digest()),
            )
        })
        .collect();
    let proof_policy = ProofMatchPolicy::new(
        artifact_target,
        artifact_configuration(&configuration),
        alpha_zeta_artifact_execution(&evidence, &model),
        artifact_trusted_items,
    )
    .unwrap();
    let policy = AuthenticatedProofExecutablePolicyV1::new(
        verifier_policy,
        proof_policy,
        manifest,
        payload(0x44),
        code_object_version,
        compiler(),
        producer(),
        DigestAlgorithm::Sha256,
    );
    let binding =
        bind_authenticated_proof_executable_persistent_v1(evidence, &policy, freshness).unwrap();
    (input, binding)
}

#[cfg(target_os = "linux")]
#[test]
fn inert_executable_join_rejects_alpha_role_over_zeta_executable() {
    let directory = PersistentLedgerDirectory::new();
    let (mut ledger, _) = PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
    let (input, binding) = alpha_zeta_persistent_binding_with_role_and_cov(
        Gfx942AlphaZetaKernelV1::Zeta,
        Gfx942AlphaZetaKernelV1::Alpha,
        "gfx942:xnack-",
        digest(0xb0),
        digest(0xb1),
        AlphaZetaIdentityMutation::Exact,
        AlphaZetaConfigurationMutation::Exact,
        ExecutableCodeObjectVersionV1::V6,
        &mut ledger,
    );
    let review = AlphaZetaExecutableEvidenceReviewV1::new(
        input.identity(),
        binding.binding_identity(),
        digest(0xb2),
        digest(0xb3),
    )
    .unwrap();
    assert_eq!(
        record_inert_alpha_zeta_executable_evidence_v1(&input, binding, review),
        Err(AlphaZetaProofErrorV1::KernelRoleSubstitution)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn inert_executable_join_rejects_code_object_v4_and_v5() {
    for (version, proof_nonce) in [
        (ExecutableCodeObjectVersionV1::V4, digest(0xb4)),
        (ExecutableCodeObjectVersionV1::V5, digest(0xb5)),
    ] {
        let directory = PersistentLedgerDirectory::new();
        let (mut ledger, _) =
            PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
        let (input, binding) = alpha_zeta_persistent_binding_with_role_and_cov(
            Gfx942AlphaZetaKernelV1::Alpha,
            Gfx942AlphaZetaKernelV1::Alpha,
            "gfx942:xnack-",
            digest(0xb6),
            proof_nonce,
            AlphaZetaIdentityMutation::Exact,
            AlphaZetaConfigurationMutation::Exact,
            version,
            &mut ledger,
        );
        let review = AlphaZetaExecutableEvidenceReviewV1::new(
            input.identity(),
            binding.binding_identity(),
            digest(0xb7),
            proof_nonce,
        )
        .unwrap();
        assert_eq!(
            record_inert_alpha_zeta_executable_evidence_v1(&input, binding, review),
            Err(AlphaZetaProofErrorV1::UnsupportedCodeObjectVersion)
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn inert_alpha_zeta_executable_evidence_set_consumes_one_contiguous_durable_lineage() {
    let directory = PersistentLedgerDirectory::new();
    let (mut ledger, _) = PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
    let proof_set_nonce = digest(0xd0);
    let (alpha_input, alpha_binding) = alpha_zeta_persistent_binding(
        Gfx942AlphaZetaKernelV1::Alpha,
        "gfx942:xnack-",
        proof_set_nonce,
        digest(0xd1),
        AlphaZetaIdentityMutation::Exact,
        AlphaZetaConfigurationMutation::Exact,
        &mut ledger,
    );
    let alpha_review = AlphaZetaExecutableEvidenceReviewV1::new(
        alpha_input.identity(),
        alpha_binding.binding_identity(),
        digest(0xe0),
        digest(0xe1),
    )
    .unwrap();
    let alpha =
        record_inert_alpha_zeta_executable_evidence_v1(&alpha_input, alpha_binding, alpha_review)
            .unwrap();

    let (zeta_input, zeta_binding) = alpha_zeta_persistent_binding(
        Gfx942AlphaZetaKernelV1::Zeta,
        "gfx942:xnack-",
        proof_set_nonce,
        digest(0xd2),
        AlphaZetaIdentityMutation::Exact,
        AlphaZetaConfigurationMutation::Exact,
        &mut ledger,
    );
    let zeta_review = AlphaZetaExecutableEvidenceReviewV1::new(
        zeta_input.identity(),
        zeta_binding.binding_identity(),
        digest(0xe0),
        digest(0xe2),
    )
    .unwrap();
    let zeta =
        record_inert_alpha_zeta_executable_evidence_v1(&zeta_input, zeta_binding, zeta_review)
            .unwrap();

    assert_eq!(
        alpha_input.canonical_target(),
        &Gfx942XnackMinusTargetIdentityV1::canonical()
    );
    assert_eq!(
        alpha.persistent_binding().freshness_receipt().generation(),
        1
    );
    assert_eq!(
        zeta.persistent_binding().freshness_receipt().generation(),
        2
    );
    let set = InertAlphaZetaExecutableEvidenceSetV1::new(zeta, alpha).unwrap();
    assert_eq!(set.alpha().kernel(), Gfx942AlphaZetaKernelV1::Alpha);
    assert_eq!(set.zeta().kernel(), Gfx942AlphaZetaKernelV1::Zeta);
    assert!(!set.grants_proof_authority());
    assert!(!set.grants_launch_authority());
}

#[cfg(target_os = "linux")]
#[test]
fn inert_alpha_zeta_executable_evidence_join_rejects_configuration_and_target_substitution() {
    let configuration_directory = PersistentLedgerDirectory::new();
    let (mut configuration_ledger, _) =
        PersistentProofFreshnessLedgerV1::create_new(&configuration_directory.path).unwrap();
    for (mutation, proof_nonce) in [
        (AlphaZetaConfigurationMutation::ProofNonce, digest(0xd4)),
        (AlphaZetaConfigurationMutation::Extra, digest(0xdd)),
    ] {
        let (input, binding) = alpha_zeta_persistent_binding(
            Gfx942AlphaZetaKernelV1::Alpha,
            "gfx942:xnack-",
            digest(0xd3),
            proof_nonce,
            AlphaZetaIdentityMutation::Exact,
            mutation,
            &mut configuration_ledger,
        );
        let review = AlphaZetaExecutableEvidenceReviewV1::new(
            input.identity(),
            binding.binding_identity(),
            digest(0xe3),
            proof_nonce,
        )
        .unwrap();
        assert_eq!(
            record_inert_alpha_zeta_executable_evidence_v1(&input, binding, review),
            Err(AlphaZetaProofErrorV1::IdentityMismatch {
                field: "authenticated proof configuration",
            })
        );
    }

    let target_directory = PersistentLedgerDirectory::new();
    let (mut target_ledger, _) =
        PersistentProofFreshnessLedgerV1::create_new(&target_directory.path).unwrap();
    let (input, binding) = alpha_zeta_persistent_binding(
        Gfx942AlphaZetaKernelV1::Alpha,
        "gfx942:xnack+",
        digest(0xd5),
        digest(0xd6),
        AlphaZetaIdentityMutation::Exact,
        AlphaZetaConfigurationMutation::Exact,
        &mut target_ledger,
    );
    let review = AlphaZetaExecutableEvidenceReviewV1::new(
        input.identity(),
        binding.binding_identity(),
        digest(0xe5),
        digest(0xe6),
    )
    .unwrap();
    assert_eq!(
        record_inert_alpha_zeta_executable_evidence_v1(&input, binding, review),
        Err(AlphaZetaProofErrorV1::TargetProfileSubstitution)
    );

    let identity_directory = PersistentLedgerDirectory::new();
    let (mut identity_ledger, _) =
        PersistentProofFreshnessLedgerV1::create_new(&identity_directory.path).unwrap();
    for (mutation, proof_nonce, expected_field) in [
        (AlphaZetaIdentityMutation::Abi, digest(0xda), "artifact ABI"),
        (
            AlphaZetaIdentityMutation::Launch,
            digest(0xdb),
            "artifact launch contract",
        ),
    ] {
        let (input, binding) = alpha_zeta_persistent_binding(
            Gfx942AlphaZetaKernelV1::Alpha,
            "gfx942:xnack-",
            digest(0xdc),
            proof_nonce,
            mutation,
            AlphaZetaConfigurationMutation::Exact,
            &mut identity_ledger,
        );
        let review = AlphaZetaExecutableEvidenceReviewV1::new(
            input.identity(),
            binding.binding_identity(),
            digest(0xed),
            proof_nonce,
        )
        .unwrap();
        assert_eq!(
            record_inert_alpha_zeta_executable_evidence_v1(&input, binding, review),
            Err(AlphaZetaProofErrorV1::IdentityMismatch {
                field: expected_field,
            })
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn inert_alpha_zeta_executable_evidence_set_rejects_reused_review_nonce() {
    let directory = PersistentLedgerDirectory::new();
    let (mut ledger, _) = PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
    let proof_set_nonce = digest(0xd7);
    let review_nonce = digest(0xe7);
    let (alpha_input, alpha_binding) = alpha_zeta_persistent_binding(
        Gfx942AlphaZetaKernelV1::Alpha,
        "gfx942:xnack-",
        proof_set_nonce,
        digest(0xd8),
        AlphaZetaIdentityMutation::Exact,
        AlphaZetaConfigurationMutation::Exact,
        &mut ledger,
    );
    let alpha_review = AlphaZetaExecutableEvidenceReviewV1::new(
        alpha_input.identity(),
        alpha_binding.binding_identity(),
        digest(0xe8),
        review_nonce,
    )
    .unwrap();
    let alpha =
        record_inert_alpha_zeta_executable_evidence_v1(&alpha_input, alpha_binding, alpha_review)
            .unwrap();
    let (zeta_input, zeta_binding) = alpha_zeta_persistent_binding(
        Gfx942AlphaZetaKernelV1::Zeta,
        "gfx942:xnack-",
        proof_set_nonce,
        digest(0xd9),
        AlphaZetaIdentityMutation::Exact,
        AlphaZetaConfigurationMutation::Exact,
        &mut ledger,
    );
    let zeta_review = AlphaZetaExecutableEvidenceReviewV1::new(
        zeta_input.identity(),
        zeta_binding.binding_identity(),
        digest(0xe8),
        review_nonce,
    )
    .unwrap();
    let zeta =
        record_inert_alpha_zeta_executable_evidence_v1(&zeta_input, zeta_binding, zeta_review)
            .unwrap();
    assert_eq!(
        InertAlphaZetaExecutableEvidenceSetV1::new(alpha, zeta),
        Err(AlphaZetaProofErrorV1::MixedProofSet)
    );
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct PersistentControlFlowBlueprint {
    request_binding: fe2o3_verifier::ControlFlowProofRequestBindingV1,
    evidence: AuthenticatedRecorderOutputV1,
    policy: AuthenticatedProofExecutablePolicyV1,
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn persistent_control_flow_blueprint(
    manifest: ManifestV1,
    kernel_id: u8,
    source_digest: u8,
    executable_digest: u8,
    source_input: (Vec<u8>, Vec<u8>, ControlFlowClaimsV1),
    base_functional_specification: Digest,
    finalized_executable_digest: PayloadDigest,
    compiler: ArtifactMeasuredToolIdentity,
    producer: ArtifactMeasuredToolIdentity,
    tools: ExecutionTools,
) -> PersistentControlFlowBlueprint {
    persistent_control_flow_blueprint_with_policy_limits(
        manifest,
        kernel_id,
        source_digest,
        executable_digest,
        source_input,
        base_functional_specification,
        finalized_executable_digest,
        compiler,
        producer,
        tools,
        10,
        10,
    )
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn persistent_control_flow_blueprint_with_policy_limits(
    manifest: ManifestV1,
    kernel_id: u8,
    source_digest: u8,
    executable_digest: u8,
    source_input: (Vec<u8>, Vec<u8>, ControlFlowClaimsV1),
    base_functional_specification: Digest,
    finalized_executable_digest: PayloadDigest,
    compiler: ArtifactMeasuredToolIdentity,
    producer: ArtifactMeasuredToolIdentity,
    tools: ExecutionTools,
    policy_timeout_seconds: u32,
    invocation_timeout_seconds: u32,
) -> PersistentControlFlowBlueprint {
    let (source_bytes, cfg_identity, claims) = source_input;
    let source = reconcile_control_flow_source_v1(&source_bytes, &cfg_identity, claims).unwrap();
    let functional_specification = derive_control_flow_functional_specification_digest_v1(
        base_functional_specification,
        &source,
    )
    .unwrap();
    let target = artifact_target_for_kernel_with_bundle(
        &manifest,
        kernel_id,
        source_digest,
        executable_digest,
        source_contracts_with_functional(payload_from_digest(functional_specification)),
        finalized_executable_digest,
        &compiler,
        &producer,
    );
    let (evidence, verifier_policy) = synthetic_recorder_output_with_tools_and_limits(
        target,
        tools,
        policy_timeout_seconds,
        invocation_timeout_seconds,
    );
    let request_binding = bind_control_flow_proof_request_v1(
        evidence.invocation_plan().request(),
        base_functional_specification,
        source,
    )
    .unwrap();
    let policy = binding_policy_with_bundle(
        manifest,
        target,
        &evidence,
        verifier_policy,
        finalized_executable_digest,
        ExecutableCodeObjectVersionV1::V6,
        compiler,
        producer,
    );
    PersistentControlFlowBlueprint {
        request_binding,
        evidence,
        policy,
    }
}

#[cfg(target_os = "linux")]
fn bind_persistent_control_flow_blueprint(
    blueprint: PersistentControlFlowBlueprint,
    ledger: &mut PersistentProofFreshnessLedgerV1,
) -> PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1 {
    let proof = bind_authenticated_proof_executable_persistent_v1(
        blueprint.evidence,
        &blueprint.policy,
        ledger,
    )
    .unwrap();
    bind_persistently_fresh_authenticated_control_flow_executable_v1(
        blueprint.request_binding,
        proof,
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
struct PersistentMultiKernelBlueprints {
    first: PersistentControlFlowBlueprint,
    first_new_execution: PersistentControlFlowBlueprint,
    second: PersistentControlFlowBlueprint,
    finalized_executable_mismatch: PersistentControlFlowBlueprint,
    target_mismatch: PersistentControlFlowBlueprint,
    compiler_mismatch: PersistentControlFlowBlueprint,
    verifier_mismatch: PersistentControlFlowBlueprint,
    policy_mismatch: PersistentControlFlowBlueprint,
}

#[cfg(target_os = "linux")]
fn persistent_multi_kernel_blueprints() -> &'static PersistentMultiKernelBlueprints {
    static BLUEPRINTS: OnceLock<PersistentMultiKernelBlueprints> = OnceLock::new();
    BLUEPRINTS.get_or_init(|| {
        let base_manifest = manifest();
        let base_compiler = compiler();
        let base_producer = producer();
        let first = || {
            persistent_control_flow_blueprint(
                base_manifest.clone(),
                0x11,
                0x22,
                0x33,
                control_flow_source_with("src/kernel.rs", 8),
                digest(0x55),
                payload(0x44),
                base_compiler.clone(),
                base_producer.clone(),
                synthetic_claimed_tools(),
            )
        };
        let second = || {
            persistent_control_flow_blueprint(
                base_manifest.clone(),
                0x12,
                0x23,
                0x34,
                control_flow_source_with("src/kernel_second.rs", 4),
                digest(0x56),
                payload(0x44),
                base_compiler.clone(),
                base_producer.clone(),
                synthetic_claimed_tools(),
            )
        };
        PersistentMultiKernelBlueprints {
            first: first(),
            first_new_execution: first(),
            second: second(),
            finalized_executable_mismatch: persistent_control_flow_blueprint(
                manifest_for_bundle("gfx942", 0x45),
                0x12,
                0x23,
                0x34,
                control_flow_source_with("src/kernel_second.rs", 4),
                digest(0x56),
                payload(0x45),
                base_compiler.clone(),
                base_producer.clone(),
                synthetic_claimed_tools(),
            ),
            target_mismatch: persistent_control_flow_blueprint(
                manifest_for_processor("gfx941"),
                0x12,
                0x23,
                0x34,
                control_flow_source_with("src/kernel_second.rs", 4),
                digest(0x56),
                payload(0x44),
                base_compiler.clone(),
                base_producer.clone(),
                synthetic_claimed_tools(),
            ),
            compiler_mismatch: persistent_control_flow_blueprint(
                base_manifest.clone(),
                0x12,
                0x23,
                0x34,
                control_flow_source_with("src/kernel_second.rs", 4),
                digest(0x56),
                payload(0x44),
                artifact_tool("rustc", "1.94.0", 0x68),
                base_producer.clone(),
                synthetic_claimed_tools(),
            ),
            verifier_mismatch: persistent_control_flow_blueprint(
                base_manifest.clone(),
                0x12,
                0x23,
                0x34,
                control_flow_source_with("src/kernel_second.rs", 4),
                digest(0x56),
                payload(0x44),
                base_compiler.clone(),
                base_producer.clone(),
                synthetic_claimed_tools_with_verifier_configuration(0x79),
            ),
            policy_mismatch: persistent_control_flow_blueprint_with_policy_limits(
                base_manifest,
                0x12,
                0x23,
                0x34,
                control_flow_source_with("src/kernel_second.rs", 4),
                digest(0x56),
                payload(0x44),
                base_compiler,
                base_producer,
                synthetic_claimed_tools(),
                20,
                10,
            ),
        }
    })
}

#[cfg(target_os = "linux")]
fn persistent_bindings_in_one_ledger(
    blueprints: Vec<PersistentControlFlowBlueprint>,
) -> Vec<PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1> {
    let directory = PersistentLedgerDirectory::new();
    let (mut ledger, _) = PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
    blueprints
        .into_iter()
        .map(|blueprint| bind_persistent_control_flow_blueprint(blueprint, &mut ledger))
        .collect()
}

#[cfg(target_os = "linux")]
fn copy_persistent_ledger(source: &std::path::Path, destination: &std::path::Path) {
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), destination.join(entry.file_name())).unwrap();
    }
}

#[derive(Clone)]
struct MultiKernelProofFixture {
    first: AuthenticatedControlFlowExecutableBindingV1,
    second: AuthenticatedControlFlowExecutableBindingV1,
}

fn multi_kernel_proof_fixture() -> &'static MultiKernelProofFixture {
    static FIXTURE: OnceLock<MultiKernelProofFixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let manifest = manifest();
        MultiKernelProofFixture {
            first: authenticated_control_flow_fixture(
                &manifest,
                0x11,
                0x22,
                0x33,
                control_flow_source_with("src/kernel.rs", 8),
                digest(0x55),
            ),
            second: authenticated_control_flow_fixture(
                &manifest,
                0x12,
                0x23,
                0x34,
                control_flow_source_with("src/kernel_second.rs", 4),
                digest(0x56),
            ),
        }
    })
}

fn authenticated_control_flow_fixture(
    manifest: &ManifestV1,
    kernel_id: u8,
    source_digest: u8,
    executable_digest: u8,
    source_input: (Vec<u8>, Vec<u8>, ControlFlowClaimsV1),
    base_functional_specification: Digest,
) -> AuthenticatedControlFlowExecutableBindingV1 {
    let (source_bytes, cfg_identity, claims) = source_input;
    let source = reconcile_control_flow_source_v1(&source_bytes, &cfg_identity, claims).unwrap();
    let functional_specification = derive_control_flow_functional_specification_digest_v1(
        base_functional_specification,
        &source,
    )
    .unwrap();
    let target = artifact_target_for_kernel(
        manifest,
        kernel_id,
        source_digest,
        executable_digest,
        source_contracts_with_functional(payload_from_digest(functional_specification)),
    );
    let (evidence, verifier_policy) = synthetic_recorder_output(target);
    let request_binding = bind_control_flow_proof_request_v1(
        evidence.invocation_plan().request(),
        base_functional_specification,
        source,
    )
    .unwrap();
    let policy = binding_policy(manifest.clone(), target, &evidence, verifier_policy);
    let proof = bind_authenticated_proof_executable_v1(
        evidence,
        &policy,
        &mut AuthenticatedExecutionFreshnessV1::new(),
    )
    .unwrap();
    bind_authenticated_control_flow_executable_v1(request_binding, proof).unwrap()
}

fn multi_kernel_admission() -> MultiKernelProofAdmissionV1 {
    let fixture = multi_kernel_proof_fixture();
    MultiKernelProofAdmissionV1::new(vec![fixture.second.clone(), fixture.first.clone()]).unwrap()
}

#[test]
fn multi_kernel_admission_binds_each_kernel_to_its_own_proof() {
    let fixture = multi_kernel_proof_fixture();
    let admission = multi_kernel_admission();
    let canonical =
        MultiKernelProofAdmissionV1::new(vec![fixture.first.clone(), fixture.second.clone()])
            .unwrap();
    let first = KernelProofAdmissionRequestV1::from_binding(&fixture.first);
    let second = KernelProofAdmissionRequestV1::from_binding(&fixture.second);

    assert_eq!(MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1, *b"FE2MKPA\0");
    assert_eq!(admission.version(), MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1);
    assert_eq!(admission.kernel_count(), 2);
    assert_eq!(admission.binding_identity(), canonical.binding_identity());
    assert_eq!(
        admission.admit_kernel(first).unwrap().binding_identity(),
        fixture.first.binding_identity()
    );
    assert_eq!(
        admission.admit_kernel(second).unwrap().binding_identity(),
        fixture.second.binding_identity()
    );
    assert_ne!(first.kernel_id(), second.kernel_id());
    assert_ne!(first.source_identity(), second.source_identity());
    assert_ne!(first.contract_identity(), second.contract_identity());
    assert_ne!(
        first.authenticated_proof_identity(),
        second.authenticated_proof_identity()
    );
    assert_eq!(
        admission.finalized_executable_digest(),
        fixture
            .first
            .proof_executable_binding()
            .executable_binding()
            .executable()
            .finalized_code_object_digest()
    );
    assert!(!admission.grants_load_authority());
    assert!(!admission.grants_launch_authority());
}

#[test]
fn multi_kernel_admission_rejects_stale_request_identity() {
    let fixture = multi_kernel_proof_fixture();
    let original = fixture.first.request_binding();
    let stale_request = ProofRequestV1::new(
        CorrelationId::from_bytes([52; 16]),
        original.target(),
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let stale = bind_control_flow_proof_request_v1(
        &stale_request,
        original.base_functional_specification_digest(),
        original.source().clone(),
    )
    .unwrap();
    let request = KernelProofAdmissionRequestV1::new(
        &stale,
        fixture.first.proof_executable_binding().binding_identity(),
    );

    assert_eq!(
        multi_kernel_admission().admit_kernel(request),
        Err(MultiKernelProofAdmissionErrorV1::IdentityMismatch {
            kernel_id: original.target().kernel_id,
            field: KernelProofAdmissionIdentityV1::ProofRequest,
        })
    );
}

#[test]
fn multi_kernel_admission_rejects_swapped_kernel_proof() {
    let fixture = multi_kernel_proof_fixture();
    let request = KernelProofAdmissionRequestV1::new(
        fixture.second.request_binding(),
        fixture.first.proof_executable_binding().binding_identity(),
    );

    assert_eq!(
        multi_kernel_admission().admit_kernel(request),
        Err(MultiKernelProofAdmissionErrorV1::IdentityMismatch {
            kernel_id: fixture.second.request_binding().target().kernel_id,
            field: KernelProofAdmissionIdentityV1::AuthenticatedProof,
        })
    );
}

#[test]
fn multi_kernel_admission_rejects_swapped_contract() {
    let fixture = multi_kernel_proof_fixture();
    let original = fixture.first.request_binding();
    let swapped_base = digest(0x57);
    let swapped_functional =
        derive_control_flow_functional_specification_digest_v1(swapped_base, original.source())
            .unwrap();
    let swapped_target = artifact_target_with_contracts(
        &manifest(),
        source_contracts_with_functional(payload_from_digest(swapped_functional)),
    );
    let swapped_request = ProofRequestV1::new(
        CorrelationId::from_bytes([51; 16]),
        verifier_target(swapped_target),
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let swapped = bind_control_flow_proof_request_v1(
        &swapped_request,
        swapped_base,
        original.source().clone(),
    )
    .unwrap();
    let request = KernelProofAdmissionRequestV1::new(
        &swapped,
        fixture.first.proof_executable_binding().binding_identity(),
    );

    assert_eq!(
        multi_kernel_admission().admit_kernel(request),
        Err(MultiKernelProofAdmissionErrorV1::IdentityMismatch {
            kernel_id: original.target().kernel_id,
            field: KernelProofAdmissionIdentityV1::Contract,
        })
    );
}

#[test]
fn multi_kernel_admission_rejects_duplicate_kernel_proofs() {
    let first = &multi_kernel_proof_fixture().first;

    assert_eq!(
        MultiKernelProofAdmissionV1::new(vec![first.clone(), first.clone()]),
        Err(MultiKernelProofAdmissionErrorV1::DuplicateKernel {
            kernel_id: first.request_binding().target().kernel_id,
        })
    );
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_multi_kernel_admission_binds_exact_per_kernel_identities_without_authority() {
    let blueprints = persistent_multi_kernel_blueprints();
    let mut bindings = persistent_bindings_in_one_ledger(vec![
        blueprints.first.clone(),
        blueprints.second.clone(),
    ]);
    let first_request = PersistentlyFreshKernelProofAdmissionRequestV1::from_binding(&bindings[0]);
    let second_request = PersistentlyFreshKernelProofAdmissionRequestV1::from_binding(&bindings[1]);
    let expected_namespace = bindings[0]
        .proof_executable_binding()
        .identity()
        .ledger_namespace();
    bindings.reverse();

    let admission = PersistentlyFreshMultiKernelProofAdmissionV1::new(bindings).unwrap();
    assert_eq!(
        PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1,
        *b"FE2PMKA\0"
    );
    assert_eq!(
        admission.version(),
        PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1
    );
    assert_eq!(admission.kernel_count(), 2);
    assert_eq!(admission.ledger_namespace(), expected_namespace);
    assert_eq!(
        admission.code_object_version(),
        ExecutableCodeObjectVersionV1::V6
    );
    assert_eq!(admission.finalized_executable_digest(), payload(0x44));
    assert!(!admission.grants_load_authority());
    assert!(!admission.grants_launch_authority());

    let first = admission.admit_kernel(first_request).unwrap();
    let second = admission.admit_kernel(second_request).unwrap();
    let original = first.request_binding();
    let persistent_proof = first.proof_executable_binding();

    let (source_bytes, cfg_identity, claims) = control_flow_source_with("src/changed_kernel.rs", 8);
    let changed_source =
        reconcile_control_flow_source_v1(&source_bytes, &cfg_identity, claims).unwrap();
    let changed_functional = derive_control_flow_functional_specification_digest_v1(
        original.base_functional_specification_digest(),
        &changed_source,
    )
    .unwrap();
    let mut changed_source_target = original.target();
    changed_source_target.functional_specification_digest = changed_functional;
    let changed_source_request = ProofRequestV1::new(
        CorrelationId::from_bytes([0x81; 16]),
        changed_source_target,
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let changed_source_binding = bind_control_flow_proof_request_v1(
        &changed_source_request,
        original.base_functional_specification_digest(),
        changed_source,
    )
    .unwrap();
    let request = PersistentlyFreshKernelProofAdmissionRequestV1::new(
        &changed_source_binding,
        persistent_proof,
    );
    assert_eq!(
        admission.admit_kernel(request),
        Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::IdentityMismatch {
                kernel_id: original.target().kernel_id,
                field: PersistentlyFreshKernelProofAdmissionIdentityV1::Source,
            }
        )
    );

    let changed_base = digest(0x82);
    let changed_contract_functional =
        derive_control_flow_functional_specification_digest_v1(changed_base, original.source())
            .unwrap();
    let mut changed_contract_target = original.target();
    changed_contract_target.functional_specification_digest = changed_contract_functional;
    let changed_contract_request = ProofRequestV1::new(
        CorrelationId::from_bytes([0x82; 16]),
        changed_contract_target,
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let changed_contract_binding = bind_control_flow_proof_request_v1(
        &changed_contract_request,
        changed_base,
        original.source().clone(),
    )
    .unwrap();
    let request = PersistentlyFreshKernelProofAdmissionRequestV1::new(
        &changed_contract_binding,
        persistent_proof,
    );
    assert_eq!(
        admission.admit_kernel(request),
        Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::IdentityMismatch {
                kernel_id: original.target().kernel_id,
                field: PersistentlyFreshKernelProofAdmissionIdentityV1::Contract,
            }
        )
    );

    let changed_request = ProofRequestV1::new(
        CorrelationId::from_bytes([0x83; 16]),
        original.target(),
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let changed_request_binding = bind_control_flow_proof_request_v1(
        &changed_request,
        original.base_functional_specification_digest(),
        original.source().clone(),
    )
    .unwrap();
    let request = PersistentlyFreshKernelProofAdmissionRequestV1::new(
        &changed_request_binding,
        persistent_proof,
    );
    assert_eq!(
        admission.admit_kernel(request),
        Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::IdentityMismatch {
                kernel_id: original.target().kernel_id,
                field: PersistentlyFreshKernelProofAdmissionIdentityV1::ProofRequest,
            }
        )
    );

    let request = PersistentlyFreshKernelProofAdmissionRequestV1::new(
        original,
        second.proof_executable_binding(),
    );
    assert_eq!(
        admission.admit_kernel(request),
        Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::IdentityMismatch {
                kernel_id: original.target().kernel_id,
                field: PersistentlyFreshKernelProofAdmissionIdentityV1::AuthenticatedProof,
            }
        )
    );

    let external = persistent_bindings_in_one_ledger(vec![blueprints.first.clone()])
        .pop()
        .unwrap();
    let request = PersistentlyFreshKernelProofAdmissionRequestV1::new(
        original,
        external.proof_executable_binding(),
    );
    assert_eq!(
        admission.admit_kernel(request),
        Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::IdentityMismatch {
                kernel_id: original.target().kernel_id,
                field: PersistentlyFreshKernelProofAdmissionIdentityV1::PersistentProof,
            }
        )
    );
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_multi_kernel_admission_rejects_mixed_ledger_namespaces() {
    let blueprints = persistent_multi_kernel_blueprints();
    let first = persistent_bindings_in_one_ledger(vec![blueprints.first.clone()])
        .pop()
        .unwrap();
    let second = persistent_bindings_in_one_ledger(vec![blueprints.second.clone()])
        .pop()
        .unwrap();

    assert_eq!(
        PersistentlyFreshMultiKernelProofAdmissionV1::new(vec![first, second]),
        Err(PersistentlyFreshMultiKernelProofAdmissionErrorV1::MixedLedgerNamespace)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_multi_kernel_admission_rejects_duplicate_generations_from_forked_local_state() {
    let blueprints = persistent_multi_kernel_blueprints();
    let first_directory = PersistentLedgerDirectory::new();
    let second_directory = PersistentLedgerDirectory::new();
    let (ledger, _) = PersistentProofFreshnessLedgerV1::create_new(&first_directory.path).unwrap();
    drop(ledger);
    copy_persistent_ledger(&first_directory.path, &second_directory.path);

    let (mut first_ledger, _) =
        PersistentProofFreshnessLedgerV1::open_existing(&first_directory.path).unwrap();
    let (mut second_ledger, _) =
        PersistentProofFreshnessLedgerV1::open_existing(&second_directory.path).unwrap();
    let first = bind_persistent_control_flow_blueprint(blueprints.first.clone(), &mut first_ledger);
    let second =
        bind_persistent_control_flow_blueprint(blueprints.second.clone(), &mut second_ledger);
    assert_eq!(
        first
            .proof_executable_binding()
            .identity()
            .ledger_namespace(),
        second
            .proof_executable_binding()
            .identity()
            .ledger_namespace()
    );

    assert_eq!(
        PersistentlyFreshMultiKernelProofAdmissionV1::new(vec![first, second]),
        Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::DuplicateLedgerGeneration {
                generation: 1,
            }
        )
    );
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_multi_kernel_admission_rejects_distinct_generations_from_divergent_histories() {
    let blueprints = persistent_multi_kernel_blueprints();
    let first_directory = PersistentLedgerDirectory::new();
    let second_directory = PersistentLedgerDirectory::new();
    let (ledger, _) = PersistentProofFreshnessLedgerV1::create_new(&first_directory.path).unwrap();
    drop(ledger);
    copy_persistent_ledger(&first_directory.path, &second_directory.path);

    let (mut first_branch, _) =
        PersistentProofFreshnessLedgerV1::open_existing(&first_directory.path).unwrap();
    let (mut second_branch, _) =
        PersistentProofFreshnessLedgerV1::open_existing(&second_directory.path).unwrap();
    let first = bind_persistent_control_flow_blueprint(blueprints.first.clone(), &mut first_branch);
    let _divergent_generation = bind_persistent_control_flow_blueprint(
        blueprints.first_new_execution.clone(),
        &mut second_branch,
    );
    let second =
        bind_persistent_control_flow_blueprint(blueprints.second.clone(), &mut second_branch);

    assert_eq!(
        PersistentlyFreshMultiKernelProofAdmissionV1::new(vec![first, second]),
        Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::LedgerHistoryMismatch {
                previous_generation: 1,
                next_generation: 2,
            }
        )
    );
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_multi_kernel_admission_rejects_duplicate_kernel_identities() {
    let blueprints = persistent_multi_kernel_blueprints();
    let mut bindings = persistent_bindings_in_one_ledger(vec![
        blueprints.first.clone(),
        blueprints.first_new_execution.clone(),
    ]);
    let kernel_id = bindings[0].request_binding().target().kernel_id;

    assert_eq!(
        PersistentlyFreshMultiKernelProofAdmissionV1::new(std::mem::take(&mut bindings)),
        Err(PersistentlyFreshMultiKernelProofAdmissionErrorV1::DuplicateKernel { kernel_id })
    );
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_multi_kernel_admission_rejects_mixed_executable_target_and_claimed_tools() {
    let blueprints = persistent_multi_kernel_blueprints();
    for (field, second) in [
        (
            "finalized executable",
            blueprints.finalized_executable_mismatch.clone(),
        ),
        ("target", blueprints.target_mismatch.clone()),
        ("compiler", blueprints.compiler_mismatch.clone()),
        (
            "claimed verifier/solver identities",
            blueprints.verifier_mismatch.clone(),
        ),
        ("verifier policy digest", blueprints.policy_mismatch.clone()),
    ] {
        let bindings = persistent_bindings_in_one_ledger(vec![blueprints.first.clone(), second]);
        assert_eq!(
            PersistentlyFreshMultiKernelProofAdmissionV1::new(bindings),
            Err(PersistentlyFreshMultiKernelProofAdmissionErrorV1::SharedBundleMismatch { field })
        );
    }
}

#[test]
fn exact_synthetic_recorder_transaction_binds_every_proof_and_executable_axis() {
    let manifest = manifest();
    let target = artifact_target(&manifest);
    let (evidence, verifier_policy) = synthetic_recorder_output(target);
    let policy = binding_policy(manifest.clone(), target, &evidence, verifier_policy);
    let replay = evidence.clone();
    let mut freshness = AuthenticatedExecutionFreshnessV1::new();

    let binding =
        bind_authenticated_proof_executable_v1(evidence, &policy, &mut freshness).unwrap();
    assert_eq!(
        AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
        *b"FE2APXB\0"
    );
    assert_eq!(
        binding.version(),
        AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1
    );
    assert_eq!(freshness.consumed_count(), 1);
    assert_eq!(
        binding.executable_binding().executable().proof_target(),
        target
    );
    assert_eq!(
        binding
            .executable_binding()
            .executable()
            .source_contracts()
            .effects_digest(),
        source_contracts().effects_digest()
    );
    assert_eq!(
        binding
            .executable_binding()
            .executable()
            .finalized_code_object_digest(),
        payload(0x44)
    );
    assert_eq!(
        binding
            .executable_binding()
            .executable()
            .code_object_version(),
        ExecutableCodeObjectVersionV1::V6
    );
    assert_eq!(
        binding.executable_binding().executable().target(),
        manifest.target()
    );
    assert_eq!(
        binding.execution_identity().result().byte_len(),
        binding.execution_evidence().result_bytes().bytes().len() as u64
    );
    assert_eq!(
        binding.execution_identity().transcript_digest(),
        binding.execution_evidence().transcript_digest()
    );
    binding.validate_against(&binding).unwrap();
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());

    assert_eq!(
        bind_authenticated_proof_executable_v1(replay, &policy, &mut freshness),
        Err(AuthenticatedProofExecutableBindingError::ChallengeReplay)
    );
    assert_eq!(freshness.consumed_count(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_binding_projects_exact_capsule_and_rejects_substitutions_and_replay() {
    let directory = PersistentLedgerDirectory::new();
    let manifest = manifest();
    let target = artifact_target(&manifest);
    let (evidence, verifier_policy) = synthetic_recorder_output(target);
    let projection_policy = verifier_policy.clone();
    let replay = evidence.clone();
    let policy = binding_policy(manifest, target, &evidence, verifier_policy);
    let (mut ledger, recovery) =
        PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
    assert_eq!(recovery, PersistentFreshnessRecoveryV1::Initialized);
    let initial_state = ledger.inspect().unwrap();

    let binding =
        bind_authenticated_proof_executable_persistent_v1(evidence, &policy, &mut ledger).unwrap();
    assert_eq!(
        PERSISTENT_AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
        *b"FE2PPXB\0"
    );
    let state = ledger.inspect().unwrap();
    assert_eq!(state.generation(), 1);
    assert_eq!(state.consumed_count(), 1);
    let persistent = binding.identity();
    let receipt = binding.freshness_receipt();
    let execution = binding.proof_binding().execution_identity();
    assert_eq!(
        persistent.proof_binding_identity(),
        binding.proof_binding().binding_identity()
    );
    assert_eq!(
        persistent.consumed_execution().challenge(),
        execution.challenge()
    );
    assert_eq!(
        persistent.consumed_execution().transcript(),
        execution.transcript_digest()
    );
    assert_eq!(
        persistent.consumed_execution().result(),
        execution.result().digest()
    );
    assert_eq!(persistent.ledger_namespace(), state.namespace());
    assert_eq!(persistent.ledger_generation(), 1);
    assert_eq!(persistent.ledger_state_identity(), state.state_identity());
    assert_eq!(receipt.namespace(), persistent.ledger_namespace());
    assert_eq!(
        receipt.previous_state_identity(),
        initial_state.state_identity()
    );
    assert_eq!(receipt.generation(), persistent.ledger_generation());
    assert_eq!(receipt.state_identity(), persistent.ledger_state_identity());
    assert_ne!(
        binding.binding_identity(),
        binding.proof_binding().binding_identity()
    );
    binding.validate_against(&binding).unwrap();
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());

    let exact_proof_target = verifier_target(target);
    let finalized_payload_identity = verifier_digest(payload(0x44));
    let capsule_target = proof_capsule_target(exact_proof_target, finalized_payload_identity);
    let capsule = ProofCapsuleV1::project_inert_from_persistently_fresh(
        capsule_target.clone(),
        &projection_policy,
        &binding,
    )
    .unwrap();
    assert_eq!(capsule.target(), &capsule_target);
    assert_eq!(
        capsule.target().finalized_payload_identity(),
        finalized_payload_identity
    );
    assert_eq!(
        capsule.execution().freshness(),
        Some(ProofCapsuleFreshnessIdentityV1::project_from_persistent(
            &binding,
        ))
    );
    assert_eq!(
        ProofCapsuleV1::from_bytes(&capsule.to_bytes()).unwrap(),
        capsule
    );

    let exact_expectation = ProofCapsuleExpectationV1::new(
        capsule.identity(),
        capsule.target().artifact_identity(),
        Some(ProofCapsuleFreshnessExpectationV1::project_from_persistent(
            &binding,
        )),
    )
    .unwrap();
    ProcessLocalProofCapsuleDuplicateDetectorV1::with_max_records(1)
        .unwrap()
        .parse_validate_and_record(&capsule.to_bytes(), exact_expectation)
        .unwrap();

    let substituted_policy = make_verifier_policy(synthetic_claimed_tools(), 9);
    assert_eq!(
        ProofCapsuleV1::project_inert_from_persistently_fresh(
            capsule_target.clone(),
            &substituted_policy,
            &binding,
        ),
        Err(ProofCapsuleBuildErrorV1::PolicyIdentityMismatch)
    );

    let substituted_proof_target = ProofTargetIdentity {
        source_tree_digest: digest(0xb0),
        ..exact_proof_target
    };
    assert_eq!(
        ProofCapsuleV1::project_inert_from_persistently_fresh(
            proof_capsule_target(substituted_proof_target, finalized_payload_identity),
            &projection_policy,
            &binding,
        ),
        Err(ProofCapsuleBuildErrorV1::ProofTargetMismatch)
    );
    assert_eq!(
        ProofCapsuleV1::project_inert_from_persistently_fresh(
            proof_capsule_target(exact_proof_target, digest(0xb1)),
            &projection_policy,
            &binding,
        ),
        Err(ProofCapsuleBuildErrorV1::FinalizedPayloadMismatch)
    );

    let exact_freshness = ProofCapsuleFreshnessIdentityV1::project_from_persistent(&binding);
    let mut substituted_previous_state =
        *exact_freshness.previous_ledger_state_identity().as_bytes();
    substituted_previous_state[0] ^= 1;
    let substituted_freshness = ProofCapsuleFreshnessIdentityV1::new_inert(
        exact_freshness.proof_binding_identity(),
        exact_freshness.challenge(),
        exact_freshness.transcript(),
        exact_freshness.result(),
        exact_freshness.ledger_namespace(),
        Digest::from_bytes(substituted_previous_state),
        exact_freshness.ledger_generation(),
        exact_freshness.ledger_state_identity(),
        exact_freshness.persistent_binding_identity(),
    )
    .unwrap();
    let substituted_expectation = ProofCapsuleExpectationV1::new(
        capsule.identity(),
        capsule.target().artifact_identity(),
        Some(ProofCapsuleFreshnessExpectationV1::new(
            substituted_freshness,
        )),
    )
    .unwrap();
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::with_max_records(1)
            .unwrap()
            .parse_validate_and_record(&capsule.to_bytes(), substituted_expectation),
        Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::PreviousLedgerState,
        })
    );
    drop(ledger);

    let (mut reopened, recovery) =
        PersistentProofFreshnessLedgerV1::open_existing(&directory.path).unwrap();
    assert_eq!(recovery, PersistentFreshnessRecoveryV1::Clean);
    assert_eq!(reopened.inspect().unwrap().generation(), 1);
    assert_eq!(
        bind_authenticated_proof_executable_persistent_v1(replay, &policy, &mut reopened),
        Err(
            AuthenticatedProofExecutableBindingError::PersistentFreshness(
                PersistentFreshnessLedgerErrorV1::Replay {
                    field: PersistentFreshnessIdentityFieldV1::Challenge,
                },
            ),
        )
    );
    assert_eq!(reopened.inspect().unwrap().consumed_count(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn persistent_control_flow_binding_retains_the_ledger_identity() {
    let directory = PersistentLedgerDirectory::new();
    let manifest = manifest();
    let (source_bytes, cfg_identity, claims) = control_flow_source();
    let source = reconcile_control_flow_source_v1(&source_bytes, &cfg_identity, claims).unwrap();
    let base_functional_specification = digest(0x55);
    let functional_specification = derive_control_flow_functional_specification_digest_v1(
        base_functional_specification,
        &source,
    )
    .unwrap();
    let target = artifact_target_with_contracts(
        &manifest,
        source_contracts_with_functional(payload_from_digest(functional_specification)),
    );
    let (evidence, verifier_policy) = synthetic_recorder_output(target);
    let request_binding = bind_control_flow_proof_request_v1(
        evidence.invocation_plan().request(),
        base_functional_specification,
        source,
    )
    .unwrap();
    let policy = binding_policy(manifest, target, &evidence, verifier_policy);
    let (mut ledger, _) = PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
    let proof =
        bind_authenticated_proof_executable_persistent_v1(evidence, &policy, &mut ledger).unwrap();
    let namespace = proof.identity().ledger_namespace();
    let generation = proof.identity().ledger_generation();

    let binding =
        bind_persistently_fresh_authenticated_control_flow_executable_v1(request_binding, proof)
            .unwrap();
    assert_eq!(
        PERSISTENT_AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
        *b"FE2PCFB\0"
    );
    assert_eq!(
        binding
            .proof_executable_binding()
            .identity()
            .ledger_namespace(),
        namespace
    );
    assert_eq!(
        binding
            .proof_executable_binding()
            .identity()
            .ledger_generation(),
        generation
    );
    binding.validate_against(&binding).unwrap();
    assert!(!binding.grants_compiler_authority());
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());
}

#[test]
fn exact_control_flow_identity_reaches_measured_result_and_final_executable() {
    let manifest = manifest();
    let (source_bytes, cfg_identity, claims) = control_flow_source();
    let source = reconcile_control_flow_source_v1(&source_bytes, &cfg_identity, claims).unwrap();
    let base_functional_specification = digest(0x55);
    let functional_specification = derive_control_flow_functional_specification_digest_v1(
        base_functional_specification,
        &source,
    )
    .unwrap();
    let target = artifact_target_with_contracts(
        &manifest,
        source_contracts_with_functional(payload_from_digest(functional_specification)),
    );
    let (evidence, verifier_policy) = synthetic_recorder_output(target);
    let request_binding = bind_control_flow_proof_request_v1(
        evidence.invocation_plan().request(),
        base_functional_specification,
        source.clone(),
    )
    .unwrap();
    let policy = binding_policy(manifest.clone(), target, &evidence, verifier_policy);
    let proof = bind_authenticated_proof_executable_v1(
        evidence,
        &policy,
        &mut AuthenticatedExecutionFreshnessV1::new(),
    )
    .unwrap();

    let stale_base = digest(0x56);
    let stale_functional =
        derive_control_flow_functional_specification_digest_v1(stale_base, &source).unwrap();
    let stale_target = artifact_target_with_contracts(
        &manifest,
        source_contracts_with_functional(payload_from_digest(stale_functional)),
    );
    let stale_request = ProofRequestV1::new(
        CorrelationId::from_bytes([51; 16]),
        verifier_target(stale_target),
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let stale_request_binding =
        bind_control_flow_proof_request_v1(&stale_request, stale_base, source).unwrap();
    assert_eq!(
        bind_authenticated_control_flow_executable_v1(stale_request_binding, proof.clone()),
        Err(ControlFlowBindingErrorV1::ProofRequestMismatch)
    );

    let binding = bind_authenticated_control_flow_executable_v1(request_binding, proof).unwrap();
    assert_eq!(
        AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
        *b"FE2ACFB\0"
    );
    assert_eq!(
        binding.request_binding().functional_specification_digest(),
        functional_specification
    );
    assert_eq!(
        binding
            .proof_executable_binding()
            .execution_identity()
            .request_digest(),
        binding.request_binding().request_digest()
    );
    assert_ne!(binding.binding_identity(), digest(0));
    binding.validate_against(&binding).unwrap();
    assert!(!binding.grants_compiler_authority());
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());

    let (second_evidence, second_verifier_policy) = synthetic_recorder_output(target);
    let second_policy = binding_policy(manifest, target, &second_evidence, second_verifier_policy);
    let second_proof = bind_authenticated_proof_executable_v1(
        second_evidence,
        &second_policy,
        &mut AuthenticatedExecutionFreshnessV1::new(),
    )
    .unwrap();
    let second = bind_authenticated_control_flow_executable_v1(
        binding.request_binding().clone(),
        second_proof,
    )
    .unwrap();
    assert_ne!(binding.binding_identity(), second.binding_identity());
    assert_eq!(
        binding.validate_against(&second),
        Err(ControlFlowBindingErrorV1::AuthenticatedExecutableBinding(
            AuthenticatedProofExecutableBindingError::IdentityMismatch { field: "challenge" }
        ))
    );
}

#[test]
fn verifier_policy_and_source_effect_substitution_fail_without_consuming_freshness() {
    let manifest = manifest();
    let target = artifact_target(&manifest);
    let (evidence, verifier_policy) = synthetic_recorder_output(target);
    let mut freshness = AuthenticatedExecutionFreshnessV1::new();

    let changed_verifier_policy = make_verifier_policy(
        verifier_policy.expected_tools().clone(),
        verifier_policy.max_timeout_seconds() - 1,
    );
    let changed = binding_policy(manifest.clone(), target, &evidence, changed_verifier_policy);
    assert_eq!(
        bind_authenticated_proof_executable_v1(evidence.clone(), &changed, &mut freshness),
        Err(AuthenticatedProofExecutableBindingError::PolicyDigestMismatch)
    );
    assert_eq!(freshness.consumed_count(), 0);

    let contracts = target.source_contracts();
    let changed_target = ArtifactTarget::new(
        target.artifact(),
        SourceContractIdentity::new(
            contracts.memory_digest(),
            payload(0xee),
            contracts.type_layout_digest(),
            contracts.capability_semantics_digest(),
            contracts.functional_specification_digest(),
        ),
    );
    let changed = AuthenticatedProofExecutablePolicyV1::new(
        verifier_policy.clone(),
        proof_policy(changed_target, &evidence),
        manifest.clone(),
        payload(0x44),
        ExecutableCodeObjectVersionV1::V6,
        compiler(),
        producer(),
        DigestAlgorithm::Sha256,
    );
    assert!(matches!(
        bind_authenticated_proof_executable_v1(evidence.clone(), &changed, &mut freshness),
        Err(AuthenticatedProofExecutableBindingError::ProofMatch(
            fe2o3_artifacts::ProofMatchError::IdentityMismatch("effect contract")
        ))
    ));
    assert_eq!(freshness.consumed_count(), 0);

    let changed = AuthenticatedProofExecutablePolicyV1::new(
        verifier_policy.clone(),
        proof_policy(target, &evidence),
        manifest.clone(),
        payload(0xef),
        ExecutableCodeObjectVersionV1::V6,
        compiler(),
        producer(),
        DigestAlgorithm::Sha256,
    );
    assert_eq!(
        bind_authenticated_proof_executable_v1(evidence.clone(), &changed, &mut freshness),
        Err(AuthenticatedProofExecutableBindingError::ExecutableBinding(
            fe2o3_artifacts::ProofExecutableBindingError::ProofTarget(
                fe2o3_artifacts::ProofTargetError::ManifestDigestMismatch("code object")
            )
        ))
    );
    assert_eq!(freshness.consumed_count(), 0);

    let substituted_compiler = artifact_tool("rustc", "1.94.0", 0xe0);
    let changed = AuthenticatedProofExecutablePolicyV1::new(
        verifier_policy.clone(),
        proof_policy(target, &evidence),
        manifest.clone(),
        payload(0x44),
        ExecutableCodeObjectVersionV1::V6,
        substituted_compiler,
        producer(),
        DigestAlgorithm::Sha256,
    );
    assert_eq!(
        bind_authenticated_proof_executable_v1(evidence.clone(), &changed, &mut freshness),
        Err(AuthenticatedProofExecutableBindingError::ExecutableBinding(
            fe2o3_artifacts::ProofExecutableBindingError::ProofTargetMismatch
        ))
    );
    assert_eq!(freshness.consumed_count(), 0);

    let exact = binding_policy(manifest, target, &evidence, verifier_policy);
    bind_authenticated_proof_executable_v1(evidence, &exact, &mut freshness).unwrap();
    assert_eq!(freshness.consumed_count(), 1);
}

#[test]
fn independent_synthetic_recorder_runs_cannot_be_substituted_for_each_other() {
    let manifest = manifest();
    let target = artifact_target(&manifest);
    let (first_evidence, first_verifier_policy) = synthetic_recorder_output(target);
    let (second_evidence, second_verifier_policy) = synthetic_recorder_output(target);
    let first_policy = binding_policy(
        manifest.clone(),
        target,
        &first_evidence,
        first_verifier_policy,
    );
    let second_policy = binding_policy(manifest, target, &second_evidence, second_verifier_policy);
    let first = bind_authenticated_proof_executable_v1(
        first_evidence,
        &first_policy,
        &mut AuthenticatedExecutionFreshnessV1::new(),
    )
    .unwrap();
    let second = bind_authenticated_proof_executable_v1(
        second_evidence,
        &second_policy,
        &mut AuthenticatedExecutionFreshnessV1::new(),
    )
    .unwrap();

    assert_ne!(
        first.execution_identity().challenge(),
        second.execution_identity().challenge()
    );
    assert_ne!(first.binding_identity(), second.binding_identity());
    assert_eq!(
        first.validate_against(&second),
        Err(AuthenticatedProofExecutableBindingError::IdentityMismatch { field: "challenge" })
    );
}

#[cfg(target_os = "linux")]
#[test]
fn caller_selected_recorder_and_alternate_ledgers_remain_non_authoritative() {
    let source_input = control_flow_source();
    let source =
        reconcile_control_flow_source_v1(&source_input.0, &source_input.1, source_input.2.clone())
            .unwrap();
    let allocation = AllocationSpecV1::new(
        AllocationProvenanceIdV1::new(7).unwrap(),
        AddressSpaceIdV1::new(3).unwrap(),
        0x1_0000,
        64,
        0x2_0000,
    )
    .unwrap();
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    let description = StaticViewDescriptionV1::describe(
        allocation,
        parent,
        16,
        3,
        4,
        4,
        4,
        StaticViewAccessDescriptionV1::ExclusiveWrite,
    )
    .unwrap();
    let lifetime = StaticViewLifetimeEpochClaimV1::new(digest(0xa0), 4, 8, 7).unwrap();
    let obligation = StaticViewProofObligationV1::new(
        description,
        &source,
        digest(0x22),
        digest(0x50),
        digest(0x52),
        digest(0x53),
        digest(0xa1),
        lifetime,
        Some(digest(0xa2)),
    )
    .unwrap();
    let base_functional_specification =
        derive_static_view_functional_specification_digest_v1(&obligation);
    let blueprint = persistent_control_flow_blueprint(
        manifest(),
        0x11,
        0x22,
        0x33,
        source_input,
        base_functional_specification,
        payload(0x44),
        compiler(),
        producer(),
        synthetic_claimed_tools(),
    );
    assert_eq!(
        blueprint.evidence.invocation_plan().request().properties(),
        STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1
    );
    assert_eq!(
        blueprint.evidence.recorder_report().outcome(),
        ProofOutcome::Proved
    );
    let static_evidence = bind_static_view_proof_evidence_v1(
        blueprint.evidence.invocation_plan().request(),
        blueprint.request_binding.clone(),
        obligation,
    )
    .unwrap();
    assert!(!static_evidence.grants_proof_authority());
    assert!(!static_evidence.grants_runtime_authority());
    assert!(!static_evidence.authenticates_verifier_execution());
    assert!(!static_evidence.authenticates_global_ledger_namespace());
    assert!(!static_evidence.authenticates_live_allocation());
    assert!(!static_evidence.authenticates_exclusive_lease());

    let alternate = blueprint.clone();
    let first_directory = PersistentLedgerDirectory::new();
    let second_directory = PersistentLedgerDirectory::new();
    let (mut first_ledger, _) =
        PersistentProofFreshnessLedgerV1::create_new(&first_directory.path).unwrap();
    let (mut second_ledger, _) =
        PersistentProofFreshnessLedgerV1::create_new(&second_directory.path).unwrap();
    let first = bind_persistent_control_flow_blueprint(blueprint, &mut first_ledger);
    let second = bind_persistent_control_flow_blueprint(alternate, &mut second_ledger);
    assert_ne!(
        first
            .proof_executable_binding()
            .identity()
            .ledger_namespace(),
        second
            .proof_executable_binding()
            .identity()
            .ledger_namespace()
    );
    assert!(!first.grants_load_authority());
    assert!(!first.grants_launch_authority());
    assert!(!second.grants_load_authority());
    assert!(!second.grants_launch_authority());
}

#[test]
fn required_property_policy_is_complete_and_canonical() {
    assert_eq!(
        V1_REQUIRED_PROPERTIES.map(|property| format!("{property:?}")),
        [
            "Bounds",
            "AddressOverflowFreedom",
            "MemorySafety",
            "Initialization",
            "RaceFreedom",
            "LaunchValidity",
            "FunctionalCorrectness",
        ]
    );
}
