use std::fs;
use std::sync::{Mutex, OnceLock};

use fe2o3_artifacts::{
    AbiLayout, BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity, CompilerIdentity,
    ConfigurationEntry as ArtifactConfigurationEntry, DigestAlgorithm, DigestBytes, Dimensions,
    Endianness, ExecutableCodeObjectVersionV1, IdentityText, KernelEntry, LaunchContract,
    ManifestV1, MeasuredToolIdentity as ArtifactMeasuredToolIdentity, Name, PayloadDigest,
    PointerWidth, ProofExecutionIdentity, ProofMatchPolicy, ProofTargetIdentity as ArtifactTarget,
    SourceContractIdentity, TargetIdentity, ToolIdentity, V1_REQUIRED_PROPERTIES,
    VerificationModelIdentity as ArtifactModel,
};
use fe2o3_rustc_front::{
    ControlFlowContractV1, ControlFlowNodeIdV1, ControlFlowNodeKindV1, ControlFlowNodeV1,
    FrontendIntegerSwitchCaseV1, FrontendIntegerSwitchTypeV1, FrontendSourceSpanV1,
    encode_control_flow_contract_v1,
};
use fe2o3_verifier::{
    AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
    AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
    AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1, AuthenticatedControlFlowExecutableBindingV1,
    AuthenticatedExecutionFreshnessV1, AuthenticatedExecutionProgramsV1,
    AuthenticatedProofExecutableBindingError, AuthenticatedProofExecutablePolicyV1,
    AuthenticatedVerusExecutionEvidenceV1, AxiomPolicy, Configuration, ConfigurationEntry,
    ControlFlowBindingErrorV1, ControlFlowClaimsV1, ControlFlowIntegerSwitchCaseClaimV1,
    ControlFlowIntegerSwitchClaimV1, ControlFlowLoopClaimV1, CorrelationId, Digest,
    ExecutionLimits, ExecutionTools, KernelProofAdmissionIdentityV1, KernelProofAdmissionRequestV1,
    MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1, MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1,
    MeasuredToolIdentity, MultiKernelProofAdmissionErrorV1, MultiKernelProofAdmissionV1,
    PERSISTENT_AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
    PERSISTENT_AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
    PersistentFreshnessIdentityFieldV1, PersistentFreshnessLedgerErrorV1,
    PersistentFreshnessRecoveryV1, PersistentProofFreshnessLedgerV1, ProofProperty, ProofRequestV1,
    ProofTargetIdentity, VerificationModelIdentity, VerifierPolicy,
    bind_authenticated_control_flow_executable_v1,
    bind_authenticated_proof_executable_persistent_v1, bind_authenticated_proof_executable_v1,
    bind_control_flow_proof_request_v1,
    bind_persistently_fresh_authenticated_control_flow_executable_v1,
    derive_control_flow_functional_specification_digest_v1, execute_authenticated_verus,
    reconcile_control_flow_source_v1,
};

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

fn fixture_program() -> &'static str {
    env!("CARGO_BIN_EXE_fe2o3-verifier-test-recorder")
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
    let target = TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx942"),
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
    manifest
        .proof_target(
            payload(kernel_id),
            payload(0x40),
            payload(source),
            payload(0x41),
            payload(executable),
            payload(0x44),
            source_contracts,
            &compiler(),
            &producer(),
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

fn configuration() -> Configuration {
    Configuration::new(vec![ConfigurationEntry::new("solver", "z3").unwrap()]).unwrap()
}

fn model() -> VerificationModelIdentity {
    VerificationModelIdentity::new("gpu-model-v1", digest(0x70)).unwrap()
}

fn measured_tool(name: &str, executable_digest: Digest, config: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(name, "test-v1", executable_digest, digest(config)).unwrap()
}

fn execution_tools() -> ExecutionTools {
    let executable_digest = sha256(&fs::read(fixture_program()).unwrap());
    ExecutionTools::new(
        measured_tool("verus", executable_digest, 0x71),
        measured_tool("z3", executable_digest, 0x72),
        measured_tool("fe2o3-recorder", executable_digest, 0x73),
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

fn measured_execution(
    target: ArtifactTarget,
) -> (AuthenticatedVerusExecutionEvidenceV1, VerifierPolicy) {
    static MEASURED_EXECUTION: Mutex<()> = Mutex::new(());

    // The debug recorder is roughly 17 MiB and deliberately self-hashes before
    // producing evidence. Its wall-clock runtime is scheduler-sensitive under
    // concurrent test load, so success fixtures serialize that expensive work
    // and use the policy's existing 10-second ceiling. Focused one-second
    // timeout coverage remains in the executor tests.
    let _execution = MEASURED_EXECUTION.lock().unwrap();
    let tools = execution_tools();
    let verifier_policy = make_verifier_policy(tools, 10);
    let request = ProofRequestV1::new(
        CorrelationId::from_bytes([51; 16]),
        verifier_target(target),
        configuration(),
        model(),
        ALL_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    let programs = AuthenticatedExecutionProgramsV1::new(
        fixture_program(),
        fixture_program(),
        fixture_program(),
    )
    .unwrap();
    let evidence = execute_authenticated_verus(
        request,
        programs,
        verifier_policy.max_timeout_seconds(),
        &verifier_policy,
        ExecutionLimits::default(),
    )
    .unwrap();
    (evidence, verifier_policy)
}

fn artifact_execution(evidence: &AuthenticatedVerusExecutionEvidenceV1) -> ProofExecutionIdentity {
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
    evidence: &AuthenticatedVerusExecutionEvidenceV1,
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
    evidence: &AuthenticatedVerusExecutionEvidenceV1,
    verifier_policy: VerifierPolicy,
) -> AuthenticatedProofExecutablePolicyV1 {
    AuthenticatedProofExecutablePolicyV1::new(
        verifier_policy,
        proof_policy(target, evidence),
        manifest,
        payload(0x44),
        ExecutableCodeObjectVersionV1::V6,
        compiler(),
        producer(),
        DigestAlgorithm::Sha256,
    )
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
    let (evidence, verifier_policy) = measured_execution(target);
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

#[test]
fn exact_measured_transaction_binds_every_proof_and_executable_axis() {
    let manifest = manifest();
    let target = artifact_target(&manifest);
    let (evidence, verifier_policy) = measured_execution(target);
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
fn persistent_binding_rejects_exact_evidence_replay_after_restart() {
    let directory = PersistentLedgerDirectory::new();
    let manifest = manifest();
    let target = artifact_target(&manifest);
    let (evidence, verifier_policy) = measured_execution(target);
    let replay = evidence.clone();
    let policy = binding_policy(manifest, target, &evidence, verifier_policy);
    let (mut ledger, recovery) =
        PersistentProofFreshnessLedgerV1::create_new(&directory.path).unwrap();
    assert_eq!(recovery, PersistentFreshnessRecoveryV1::Initialized);

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
    assert_eq!(receipt.generation(), persistent.ledger_generation());
    assert_eq!(receipt.state_identity(), persistent.ledger_state_identity());
    assert_ne!(
        binding.binding_identity(),
        binding.proof_binding().binding_identity()
    );
    binding.validate_against(&binding).unwrap();
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());
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
    let (evidence, verifier_policy) = measured_execution(target);
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
    let (evidence, verifier_policy) = measured_execution(target);
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

    let (second_evidence, second_verifier_policy) = measured_execution(target);
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
    let (evidence, verifier_policy) = measured_execution(target);
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
fn independent_measured_runs_cannot_be_substituted_for_each_other() {
    let manifest = manifest();
    let target = artifact_target(&manifest);
    let (first_evidence, first_verifier_policy) = measured_execution(target);
    let (second_evidence, second_verifier_policy) = measured_execution(target);
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
