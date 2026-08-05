#[allow(dead_code)]
mod common;

use common::{digest, manifest, name, text};
use fe2o3_artifacts::{
    AbiLayout, Capability, CodeObjectFormat, CodeObjectIdentity, CompilerIdentity,
    ConfigurationEntry, DigestAlgorithm, Endianness, KernelEntry, LaunchContract, ManifestV1,
    MatchedProofEvidenceV1, MeasuredToolIdentity, PayloadDigest, PointerWidth,
    ProofArtifactIdentity, ProofExecutionIdentity, ProofMatchError, ProofMatchPolicy, ProofOutcome,
    ProofProperty, ProofRecordV1, ProofTargetError, ProofTargetIdentity, SourceContractIdentity,
    TargetIdentity, ToolIdentity, TrustedItem, V1_REQUIRED_PROPERTIES, VerificationModelIdentity,
};

fn sha(byte: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(byte))
}

fn measured_tool(name: &str, version: &str, byte: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(text(name), text(version), sha(byte), sha(byte + 1))
}

fn compiler() -> MeasuredToolIdentity {
    measured_tool("rustc", "1.94.0", 0x60)
}

fn producer() -> MeasuredToolIdentity {
    measured_tool("fe2o3", "0.1.0", 0x62)
}

fn source_contracts() -> SourceContractIdentity {
    SourceContractIdentity::new(sha(0x50), sha(0x51), sha(0x52), sha(0x53), sha(0x54))
}

fn target() -> ProofTargetIdentity {
    proof_target_for(&manifest(), &compiler(), &producer()).unwrap()
}

fn proof_target_for(
    manifest: &ManifestV1,
    compiler: &MeasuredToolIdentity,
    producer: &MeasuredToolIdentity,
) -> Result<ProofTargetIdentity, ProofTargetError> {
    proof_target_with(
        manifest,
        sha(0x11),
        sha(0x22),
        sha(0x33),
        sha(0x44),
        compiler,
        producer,
    )
}

#[allow(clippy::too_many_arguments)]
fn proof_target_with(
    manifest: &ManifestV1,
    kernel_id: PayloadDigest,
    source_tree: PayloadDigest,
    executable: PayloadDigest,
    code_object: PayloadDigest,
    compiler: &MeasuredToolIdentity,
    producer: &MeasuredToolIdentity,
) -> Result<ProofTargetIdentity, ProofTargetError> {
    manifest.proof_target(
        kernel_id,
        sha(0x40),
        source_tree,
        sha(0x41),
        executable,
        code_object,
        source_contracts(),
        compiler,
        producer,
        DigestAlgorithm::Sha256,
    )
}

fn execution() -> ProofExecutionIdentity {
    ProofExecutionIdentity::new(
        VerificationModelIdentity::new(text("fe2o3-gpu-v1"), sha(0x70)),
        measured_tool("verus", "0.2026.08.04", 0x71),
        measured_tool("z3", "4.15.2", 0x73),
        measured_tool("fe2o3-proof-driver", "0.1.0", 0x75),
        sha(0x77),
    )
}

fn configuration() -> Vec<ConfigurationEntry> {
    vec![ConfigurationEntry::new(name("feature_checked"), text("on"))]
}

fn properties() -> Vec<ProofProperty> {
    V1_REQUIRED_PROPERTIES.to_vec()
}

fn record_with(
    target: ProofTargetIdentity,
    configuration: Vec<ConfigurationEntry>,
    execution: ProofExecutionIdentity,
    outcome: ProofOutcome,
    properties: Vec<ProofProperty>,
    trusted_items: Vec<TrustedItem>,
) -> ProofRecordV1 {
    ProofRecordV1::new(
        target,
        configuration,
        execution,
        outcome,
        properties,
        trusted_items,
    )
    .unwrap()
}

fn record(target: ProofTargetIdentity, trusted_items: Vec<TrustedItem>) -> ProofRecordV1 {
    record_with(
        target,
        configuration(),
        execution(),
        ProofOutcome::Proved,
        properties(),
        trusted_items,
    )
}

fn policy(trusted_items: Vec<TrustedItem>) -> ProofMatchPolicy {
    ProofMatchPolicy::new(target(), configuration(), execution(), trusted_items).unwrap()
}

#[test]
fn exact_complete_record_matches_as_evidence_without_creating_assurance() {
    let record = record(target(), vec![]);
    let expected_digest = record.digest(DigestAlgorithm::Sha256);
    let matched: MatchedProofEvidenceV1 = policy(vec![])
        .match_record(record, DigestAlgorithm::Sha256)
        .unwrap();

    assert_eq!(matched.record_digest(), expected_digest);
    assert_eq!(matched.record().target(), target());
}

#[test]
fn every_stale_target_identity_is_rejected_independently() {
    let expected = target();
    let artifact = expected.artifact();
    let contracts = expected.source_contracts();
    let stale = [
        (
            "kernel",
            target_with(artifact_with(&artifact, 0, sha(0x80)), contracts),
        ),
        (
            "kernel instance",
            target_with(artifact_with(&artifact, 1, sha(0x81)), contracts),
        ),
        (
            "source tree",
            target_with(artifact_with(&artifact, 2, sha(0x82)), contracts),
        ),
        (
            "crate graph",
            target_with(artifact_with(&artifact, 3, sha(0x83)), contracts),
        ),
        (
            "executable semantic",
            target_with(artifact_with(&artifact, 4, sha(0x84)), contracts),
        ),
        (
            "compiler and target environment",
            target_with(artifact_with(&artifact, 5, sha(0x85)), contracts),
        ),
        (
            "artifact selection",
            target_with(artifact_with(&artifact, 6, sha(0x86)), contracts),
        ),
        (
            "artifact contract",
            target_with(artifact_with(&artifact, 7, sha(0x87)), contracts),
        ),
        (
            "memory contract",
            target_with(artifact, contracts_with(&contracts, 0, sha(0x88))),
        ),
        (
            "effect contract",
            target_with(artifact, contracts_with(&contracts, 1, sha(0x89))),
        ),
        (
            "type layout contract",
            target_with(artifact, contracts_with(&contracts, 2, sha(0x8a))),
        ),
        (
            "capability semantics contract",
            target_with(artifact, contracts_with(&contracts, 3, sha(0x8b))),
        ),
        (
            "functional specification contract",
            target_with(artifact, contracts_with(&contracts, 4, sha(0x8c))),
        ),
    ];

    for (field, stale_target) in stale {
        assert_eq!(
            policy(vec![]).match_record(record(stale_target, vec![]), DigestAlgorithm::Sha256),
            Err(ProofMatchError::IdentityMismatch(field))
        );
    }
}

#[test]
fn stale_configuration_model_tools_and_invocation_are_rejected_independently() {
    let stale_configuration = record_with(
        target(),
        vec![ConfigurationEntry::new(
            name("feature_checked"),
            text("off"),
        )],
        execution(),
        ProofOutcome::Proved,
        properties(),
        vec![],
    );
    assert_eq!(
        policy(vec![]).match_record(stale_configuration, DigestAlgorithm::Sha256),
        Err(ProofMatchError::IdentityMismatch("configuration"))
    );

    let base = execution();
    let executions = [
        (
            "verification model",
            ProofExecutionIdentity::new(
                VerificationModelIdentity::new(text("fe2o3-gpu-v2"), sha(0x70)),
                base.verifier().clone(),
                base.solver().clone(),
                base.evidence_recorder().clone(),
                base.invocation_digest(),
            ),
        ),
        (
            "verification model",
            ProofExecutionIdentity::new(
                VerificationModelIdentity::new(text("fe2o3-gpu-v1"), sha(0x78)),
                base.verifier().clone(),
                base.solver().clone(),
                base.evidence_recorder().clone(),
                base.invocation_digest(),
            ),
        ),
        measured_execution_mutation(&base, "verifier", 0, text("other-verifier"), sha(0x71)),
        measured_execution_mutation(&base, "verifier", 1, text("stale"), sha(0x71)),
        measured_execution_mutation(&base, "verifier", 2, text("unused"), sha(0x79)),
        measured_execution_mutation(&base, "verifier", 3, text("unused"), sha(0x7a)),
        measured_execution_mutation(&base, "solver", 0, text("other-solver"), sha(0x73)),
        measured_execution_mutation(&base, "solver", 1, text("stale"), sha(0x73)),
        measured_execution_mutation(&base, "solver", 2, text("unused"), sha(0x7b)),
        measured_execution_mutation(&base, "solver", 3, text("unused"), sha(0x7c)),
        measured_execution_mutation(
            &base,
            "evidence recorder",
            0,
            text("other-driver"),
            sha(0x75),
        ),
        measured_execution_mutation(&base, "evidence recorder", 1, text("stale"), sha(0x75)),
        measured_execution_mutation(&base, "evidence recorder", 2, text("unused"), sha(0x7d)),
        measured_execution_mutation(&base, "evidence recorder", 3, text("unused"), sha(0x7e)),
        (
            "proof invocation",
            ProofExecutionIdentity::new(
                base.model().clone(),
                base.verifier().clone(),
                base.solver().clone(),
                base.evidence_recorder().clone(),
                sha(0x7f),
            ),
        ),
    ];

    for (field, stale_execution) in executions {
        let stale = record_with(
            target(),
            configuration(),
            stale_execution,
            ProofOutcome::Proved,
            properties(),
            vec![],
        );
        assert_eq!(
            policy(vec![]).match_record(stale, DigestAlgorithm::Sha256),
            Err(ProofMatchError::IdentityMismatch(field))
        );
    }
}

#[test]
fn incomplete_proof_missing_claims_and_trust_mismatches_fail_closed() {
    for outcome in [ProofOutcome::Failed, ProofOutcome::TimedOut] {
        let record = record_with(
            target(),
            configuration(),
            execution(),
            outcome,
            vec![],
            vec![],
        );
        assert_eq!(
            policy(vec![]).match_record(record, DigestAlgorithm::Sha256),
            Err(ProofMatchError::ProofNotComplete(outcome))
        );
    }

    for missing in V1_REQUIRED_PROPERTIES {
        let properties = properties()
            .into_iter()
            .filter(|property| *property != missing)
            .collect();
        let record = record_with(
            target(),
            configuration(),
            execution(),
            ProofOutcome::Proved,
            properties,
            vec![],
        );
        assert_eq!(
            policy(vec![]).match_record(record, DigestAlgorithm::Sha256),
            Err(ProofMatchError::MissingProperty(missing))
        );
    }

    let trusted = TrustedItem::new(name("reviewed_axiom"), sha(0xa0));
    assert_eq!(
        policy(vec![]).match_record(
            record(target(), vec![trusted.clone()]),
            DigestAlgorithm::Sha256
        ),
        Err(ProofMatchError::TrustedItemsMismatch)
    );
    assert_eq!(
        policy(vec![trusted]).match_record(record(target(), vec![]), DigestAlgorithm::Sha256),
        Err(ProofMatchError::TrustedItemsMismatch)
    );
}

#[test]
fn manifest_digest_and_tool_mismatches_fail_closed() {
    let base = manifest();
    assert_eq!(
        proof_target_with(
            &base,
            sha(0xff),
            sha(0x22),
            sha(0x33),
            sha(0x44),
            &compiler(),
            &producer(),
        ),
        Err(ProofTargetError::UnknownKernel(digest(0xff)))
    );

    for (field, source, executable, code_object) in [
        ("source tree", sha(0xfe), sha(0x33), sha(0x44)),
        ("executable semantic", sha(0x22), sha(0xfd), sha(0x44)),
        ("code object", sha(0x22), sha(0x33), sha(0xfc)),
    ] {
        assert_eq!(
            proof_target_with(
                &base,
                sha(0x11),
                source,
                executable,
                code_object,
                &compiler(),
                &producer(),
            ),
            Err(ProofTargetError::ManifestDigestMismatch(field))
        );
    }

    let tool_mismatches = [
        (
            "compiler",
            measured_tool("other-rustc", "1.94.0", 0x60),
            producer(),
        ),
        (
            "compiler",
            measured_tool("rustc", "different", 0x60),
            producer(),
        ),
        (
            "artifact producer",
            compiler(),
            measured_tool("other-producer", "0.1.0", 0x62),
        ),
        (
            "artifact producer",
            compiler(),
            measured_tool("fe2o3", "different", 0x62),
        ),
    ];
    for (field, compiler, producer) in tool_mismatches {
        assert_eq!(
            proof_target_for(&base, &compiler, &producer),
            Err(ProofTargetError::ManifestToolMismatch(field))
        );
    }
}

#[test]
fn environment_identity_covers_every_compiler_and_producer_measurement() {
    let base = manifest();
    let expected = target().artifact().environment_digest();
    let variants = [
        MeasuredToolIdentity::new(text("rustc"), text("1.94.0"), sha(0xa1), sha(0x61)),
        MeasuredToolIdentity::new(text("rustc"), text("1.94.0"), sha(0x60), sha(0xa2)),
    ];
    for changed_compiler in variants {
        assert_ne!(
            expected,
            proof_target_for(&base, &changed_compiler, &producer())
                .unwrap()
                .artifact()
                .environment_digest()
        );
    }
    let variants = [
        MeasuredToolIdentity::new(text("fe2o3"), text("0.1.0"), sha(0xa3), sha(0x63)),
        MeasuredToolIdentity::new(text("fe2o3"), text("0.1.0"), sha(0x62), sha(0xa4)),
    ];
    for changed_producer in variants {
        assert_ne!(
            expected,
            proof_target_for(&base, &compiler(), &changed_producer)
                .unwrap()
                .artifact()
                .environment_digest()
        );
    }

    let changed_manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("different")),
        ToolIdentity::new(text("fe2o3"), text("different")),
        TargetIdentity::new(
            text("amdgcn-amd-amdhsa"),
            text("gfx1151"),
            PointerWidth::Bits64,
            Endianness::Little,
            vec![Capability::AmdWave, Capability::Atomics],
        )
        .unwrap(),
        base.code_objects().to_vec(),
        base.kernels().to_vec(),
    )
    .unwrap();
    let changed_environment = proof_target_for(
        &changed_manifest,
        &measured_tool("rustc", "different", 0xa5),
        &measured_tool("fe2o3", "different", 0xa7),
    )
    .unwrap()
    .artifact()
    .environment_digest();
    assert_ne!(expected, changed_environment);
}

#[test]
fn selection_identity_includes_symbol_tagged_digest_format_and_length() {
    let base = manifest();
    let expected = target().artifact().artifact_selection_digest();

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"FE2O3SL\0");
    canonical.extend_from_slice(&1_u16.to_le_bytes());
    canonical.extend_from_slice(&0_u16.to_le_bytes());
    canonical.extend_from_slice(&13_u16.to_le_bytes());
    canonical.extend_from_slice(b"vector_add.kd");
    canonical.push(0); // SHA-256 tag.
    canonical.extend_from_slice(&[0x44; 32]);
    canonical.push(0); // NativeExecutable tag.
    canonical.extend_from_slice(&12_345_u64.to_le_bytes());
    assert_eq!(
        expected,
        DigestAlgorithm::Sha256.calculate(&canonical),
        "selection preimage must include the code-object digest algorithm"
    );

    let base_kernel = &base.kernels()[0];
    let changed_symbol = rebuild_kernel(
        base_kernel,
        base_kernel.name().clone(),
        name("different.kd"),
        base_kernel.required_capabilities().to_vec(),
        base_kernel.launch().clone(),
        base_kernel.abi().clone(),
    );
    let changed_symbol_manifest =
        rebuild_manifest(&base, changed_symbol, base.code_objects().to_vec());
    assert_ne!(
        expected,
        proof_target_for(&changed_symbol_manifest, &compiler(), &producer())
            .unwrap()
            .artifact()
            .artifact_selection_digest()
    );

    for code_object in [
        CodeObjectIdentity::new(digest(0x44), CodeObjectFormat::RelocatableObject, 12_345).unwrap(),
        CodeObjectIdentity::new(digest(0x44), CodeObjectFormat::NativeExecutable, 12_346).unwrap(),
    ] {
        let changed = rebuild_manifest(&base, base_kernel.clone(), vec![code_object]);
        assert_ne!(
            expected,
            proof_target_for(&changed, &compiler(), &producer())
                .unwrap()
                .artifact()
                .artifact_selection_digest()
        );
    }
}

#[test]
fn artifact_contract_identity_covers_name_capabilities_launch_and_abi() {
    let base = manifest();
    let base_kernel = &base.kernels()[0];
    let expected = target().artifact().artifact_contract_digest();
    let variants = [
        rebuild_kernel(
            base_kernel,
            name("different_name"),
            base_kernel.symbol().clone(),
            base_kernel.required_capabilities().to_vec(),
            base_kernel.launch().clone(),
            base_kernel.abi().clone(),
        ),
        rebuild_kernel(
            base_kernel,
            base_kernel.name().clone(),
            base_kernel.symbol().clone(),
            vec![Capability::Atomics],
            base_kernel.launch().clone(),
            base_kernel.abi().clone(),
        ),
        rebuild_kernel(
            base_kernel,
            base_kernel.name().clone(),
            base_kernel.symbol().clone(),
            base_kernel.required_capabilities().to_vec(),
            LaunchContract::new(
                base_kernel.launch().rank(),
                base_kernel.launch().block_size(),
                base_kernel.launch().max_grid(),
                base_kernel.launch().static_shared_memory_bytes(),
                base_kernel.launch().max_dynamic_shared_memory_bytes() + 1,
            )
            .unwrap(),
            base_kernel.abi().clone(),
        ),
        rebuild_kernel(
            base_kernel,
            base_kernel.name().clone(),
            base_kernel.symbol().clone(),
            base_kernel.required_capabilities().to_vec(),
            base_kernel.launch().clone(),
            AbiLayout::new(
                base_kernel.abi().size() + 8,
                base_kernel.abi().alignment(),
                base_kernel.abi().pointer_width(),
                base_kernel.abi().fields().to_vec(),
            )
            .unwrap(),
        ),
    ];

    for kernel in variants {
        let changed = rebuild_manifest(&base, kernel, base.code_objects().to_vec());
        assert_ne!(
            expected,
            proof_target_for(&changed, &compiler(), &producer())
                .unwrap()
                .artifact()
                .artifact_contract_digest()
        );
    }
}

fn measured_execution_mutation(
    base: &ProofExecutionIdentity,
    field: &'static str,
    component: usize,
    text_replacement: fe2o3_artifacts::IdentityText,
    digest_replacement: PayloadDigest,
) -> (&'static str, ProofExecutionIdentity) {
    let replacement = |tool: &MeasuredToolIdentity| {
        MeasuredToolIdentity::new(
            if component == 0 {
                text_replacement.clone()
            } else {
                tool.name().clone()
            },
            if component == 1 {
                text_replacement.clone()
            } else {
                tool.version().clone()
            },
            if component == 2 {
                digest_replacement
            } else {
                tool.executable_digest()
            },
            if component == 3 {
                digest_replacement
            } else {
                tool.configuration_digest()
            },
        )
    };
    let verifier = if field == "verifier" {
        replacement(base.verifier())
    } else {
        base.verifier().clone()
    };
    let solver = if field == "solver" {
        replacement(base.solver())
    } else {
        base.solver().clone()
    };
    let recorder = if field == "evidence recorder" {
        replacement(base.evidence_recorder())
    } else {
        base.evidence_recorder().clone()
    };
    (
        field,
        ProofExecutionIdentity::new(
            base.model().clone(),
            verifier,
            solver,
            recorder,
            base.invocation_digest(),
        ),
    )
}

fn target_with(
    artifact: ProofArtifactIdentity,
    contracts: SourceContractIdentity,
) -> ProofTargetIdentity {
    ProofTargetIdentity::new(artifact, contracts)
}

fn artifact_with(
    value: &ProofArtifactIdentity,
    index: usize,
    replacement: PayloadDigest,
) -> ProofArtifactIdentity {
    let mut fields = [
        value.kernel_id(),
        value.instance_digest(),
        value.source_tree_digest(),
        value.crate_graph_digest(),
        value.executable_digest(),
        value.environment_digest(),
        value.artifact_selection_digest(),
        value.artifact_contract_digest(),
    ];
    fields[index] = replacement;
    ProofArtifactIdentity::new(
        fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6], fields[7],
    )
}

fn contracts_with(
    value: &SourceContractIdentity,
    index: usize,
    replacement: PayloadDigest,
) -> SourceContractIdentity {
    let mut fields = [
        value.memory_digest(),
        value.effects_digest(),
        value.type_layout_digest(),
        value.capability_semantics_digest(),
        value.functional_specification_digest(),
    ];
    fields[index] = replacement;
    SourceContractIdentity::new(fields[0], fields[1], fields[2], fields[3], fields[4])
}

fn rebuild_manifest(
    base: &ManifestV1,
    kernel: KernelEntry,
    code_objects: Vec<CodeObjectIdentity>,
) -> ManifestV1 {
    ManifestV1::new(
        base.compiler().clone(),
        base.producer().clone(),
        base.target().clone(),
        code_objects,
        vec![kernel],
    )
    .unwrap()
}

fn rebuild_kernel(
    base: &KernelEntry,
    logical_name: fe2o3_artifacts::Name,
    symbol: fe2o3_artifacts::Name,
    capabilities: Vec<Capability>,
    launch: LaunchContract,
    abi: AbiLayout,
) -> KernelEntry {
    KernelEntry::new(
        base.kernel_id(),
        logical_name,
        symbol,
        base.source_digest(),
        base.executable_digest(),
        base.code_object_digest(),
        capabilities,
        launch,
        abi,
    )
    .unwrap()
}
