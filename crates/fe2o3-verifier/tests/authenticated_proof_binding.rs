use std::fs;

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
    AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1, AuthenticatedExecutionFreshnessV1,
    AuthenticatedExecutionProgramsV1, AuthenticatedProofExecutableBindingError,
    AuthenticatedProofExecutablePolicyV1, AuthenticatedVerusExecutionEvidenceV1, AxiomPolicy,
    Configuration, ConfigurationEntry, ControlFlowBindingErrorV1, ControlFlowClaimsV1,
    ControlFlowIntegerSwitchCaseClaimV1, ControlFlowIntegerSwitchClaimV1, ControlFlowLoopClaimV1,
    CorrelationId, Digest, ExecutionLimits, ExecutionTools, MeasuredToolIdentity, ProofProperty,
    ProofRequestV1, ProofTargetIdentity, VerificationModelIdentity, VerifierPolicy,
    bind_authenticated_control_flow_executable_v1, bind_authenticated_proof_executable_v1,
    bind_control_flow_proof_request_v1, derive_control_flow_functional_specification_digest_v1,
    execute_authenticated_verus, reconcile_control_flow_source_v1,
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
    let kernel = KernelEntry::new(
        bytes(0x11),
        name("verified_kernel"),
        name("verified_kernel.kd"),
        bytes(0x22),
        bytes(0x33),
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
        vec![kernel],
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
    manifest
        .proof_target(
            payload(0x11),
            payload(0x40),
            payload(0x22),
            payload(0x41),
            payload(0x33),
            payload(0x44),
            source_contracts,
            &compiler(),
            &producer(),
            DigestAlgorithm::Sha256,
        )
        .unwrap()
}

fn control_flow_source() -> (Vec<u8>, Vec<u8>, ControlFlowClaimsV1) {
    let id = ControlFlowNodeIdV1::new;
    let span = |line| FrontendSourceSpanV1::new("src/kernel.rs", line, 1, line, 8).unwrap();
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
                    max_iterations: 8,
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
        vec![ControlFlowLoopClaimV1::new(1, 8).unwrap()],
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
        2,
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
