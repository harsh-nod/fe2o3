#[allow(dead_code)]
mod common;

use common::{digest, manifest, name, text, type_identity};
use fe2o3_artifacts::{
    AbiField, AbiLayout, BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity,
    CompilerIdentity, ConfigurationEntry, DigestAlgorithm, Dimensions, Endianness,
    ExecutableCodeObjectVersionV1, KernelEntry, LaunchContract, ManifestV1, MatchedProofEvidenceV1,
    MeasuredToolIdentity, PROOF_EXECUTABLE_BINDING_DOMAIN_V1, PROOF_EXECUTABLE_BINDING_VERSION_V1,
    PayloadDigest, ProofExecutableBindingError, ProofExecutableBindingV1, ProofExecutionIdentity,
    ProofMatchPolicy, ProofOutcome, ProofRecordV1, ProofTargetIdentity, SourceContractIdentity,
    TargetIdentity, ToolIdentity, TrustedItem, V1_REQUIRED_PROPERTIES, VerificationModelIdentity,
};

fn sha(byte: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(byte))
}

fn tagged(bytes: fe2o3_artifacts::DigestBytes) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, bytes)
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

fn execution() -> ProofExecutionIdentity {
    ProofExecutionIdentity::new(
        VerificationModelIdentity::new(text("fe2o3-gpu-v1"), sha(0x70)),
        measured_tool("verus", "0.2026.08.04", 0x71),
        measured_tool("z3", "4.15.2", 0x73),
        measured_tool("fe2o3-proof-driver", "0.1.0", 0x75),
        sha(0x77),
    )
}

fn source_contracts() -> SourceContractIdentity {
    SourceContractIdentity::new(sha(0x50), sha(0x51), sha(0x52), sha(0x53), sha(0x54))
}

#[derive(Clone)]
struct Fixture {
    manifest: ManifestV1,
    compiler: MeasuredToolIdentity,
    producer: MeasuredToolIdentity,
    execution: ProofExecutionIdentity,
    configuration: Vec<ConfigurationEntry>,
    trusted_items: Vec<TrustedItem>,
    code_object_version: ExecutableCodeObjectVersionV1,
}

impl Fixture {
    fn base() -> Self {
        Self {
            manifest: manifest(),
            compiler: compiler(),
            producer: producer(),
            execution: execution(),
            configuration: vec![ConfigurationEntry::new(name("feature_checked"), text("on"))],
            trusted_items: vec![],
            code_object_version: ExecutableCodeObjectVersionV1::V5,
        }
    }

    fn kernel(&self) -> &KernelEntry {
        &self.manifest.kernels()[0]
    }

    fn finalized_digest(&self) -> PayloadDigest {
        tagged(self.kernel().code_object_digest())
    }

    fn target(&self) -> ProofTargetIdentity {
        let kernel = self.kernel();
        self.manifest
            .proof_target(
                tagged(kernel.kernel_id()),
                sha(0x40),
                tagged(kernel.source_digest()),
                sha(0x41),
                tagged(kernel.executable_digest()),
                self.finalized_digest(),
                source_contracts(),
                &self.compiler,
                &self.producer,
                DigestAlgorithm::Sha256,
            )
            .unwrap()
    }

    fn matched(&self) -> MatchedProofEvidenceV1 {
        let target = self.target();
        let record = ProofRecordV1::new(
            target,
            self.configuration.clone(),
            self.execution.clone(),
            ProofOutcome::Proved,
            V1_REQUIRED_PROPERTIES.to_vec(),
            self.trusted_items.clone(),
        )
        .unwrap();
        ProofMatchPolicy::new(
            target,
            self.configuration.clone(),
            self.execution.clone(),
            self.trusted_items.clone(),
        )
        .unwrap()
        .match_record(record, DigestAlgorithm::Sha256)
        .unwrap()
    }

    fn binding(&self) -> ProofExecutableBindingV1 {
        self.matched()
            .bind_finalized_executable_v1(
                &self.manifest,
                self.finalized_digest(),
                self.code_object_version,
                &self.compiler,
                &self.producer,
                DigestAlgorithm::Sha256,
            )
            .unwrap()
    }
}

#[test]
fn exact_matched_proof_binds_every_executable_and_policy_axis_without_authority() {
    let fixture = Fixture::base();
    let binding = fixture.binding();
    let kernel = fixture.kernel();

    assert_eq!(binding.version(), PROOF_EXECUTABLE_BINDING_VERSION_V1);
    assert_eq!(PROOF_EXECUTABLE_BINDING_DOMAIN_V1, *b"FE2OPXB\0");
    assert_eq!(
        binding.executable().kernel_semantic_identity(),
        tagged(kernel.executable_digest())
    );
    assert_eq!(
        binding.executable().finalized_code_object_digest(),
        fixture.finalized_digest()
    );
    assert_eq!(binding.executable().target(), fixture.manifest.target());
    assert_eq!(
        binding.executable().code_object_version(),
        ExecutableCodeObjectVersionV1::V5
    );
    assert_eq!(binding.executable().abi(), kernel.abi());
    assert_eq!(binding.executable().launch(), kernel.launch());
    assert_eq!(binding.tool_policy().compiler(), &fixture.compiler);
    assert_eq!(binding.tool_policy().artifact_producer(), &fixture.producer);
    assert_eq!(binding.tool_policy().proof_execution(), &fixture.execution);
    assert_eq!(
        binding.proof_record_digest(),
        fixture.matched().record_digest()
    );
    assert_eq!(
        binding,
        fixture.binding(),
        "construction must be deterministic"
    );
    binding.validate_against(&fixture.binding()).unwrap();
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());
}

#[test]
fn semantic_code_object_target_cov_abi_and_launch_mutations_fail_closed() {
    let base = Fixture::base();
    let expected = base.binding();

    let mut changed = base.clone();
    changed.manifest = manifest_with_kernel(
        kernel_with(
            base.kernel(),
            digest(0x91),
            base.kernel().code_object_digest(),
            base.kernel().abi().clone(),
            base.kernel().launch().clone(),
        ),
        base.manifest.target().clone(),
        base.manifest.code_objects().to_vec(),
        &changed.compiler,
        &changed.producer,
    );
    assert_axis_mismatch(&expected, &changed.binding(), "kernel semantic identity");

    let mut changed = base.clone();
    let changed_object = CodeObjectIdentity::new(
        digest(0x92),
        CodeObjectFormat::NativeExecutable,
        base.manifest.code_objects()[0].byte_len(),
    )
    .unwrap();
    changed.manifest = manifest_with_kernel(
        kernel_with(
            base.kernel(),
            base.kernel().executable_digest(),
            changed_object.digest(),
            base.kernel().abi().clone(),
            base.kernel().launch().clone(),
        ),
        base.manifest.target().clone(),
        vec![changed_object],
        &changed.compiler,
        &changed.producer,
    );
    assert_axis_mismatch(
        &expected,
        &changed.binding(),
        "finalized code-object digest",
    );

    for target in target_mutations(base.manifest.target()) {
        let mut changed = base.clone();
        changed.manifest = manifest_with_kernel(
            base.kernel().clone(),
            target,
            base.manifest.code_objects().to_vec(),
            &changed.compiler,
            &changed.producer,
        );
        assert_axis_mismatch(&expected, &changed.binding(), "target");
    }

    for code_object_version in [
        ExecutableCodeObjectVersionV1::V4,
        ExecutableCodeObjectVersionV1::V6,
    ] {
        let mut changed = base.clone();
        changed.code_object_version = code_object_version;
        assert_axis_mismatch(&expected, &changed.binding(), "code-object version");
    }

    for abi in abi_mutations(base.kernel().abi()) {
        let mut changed = base.clone();
        changed.manifest = manifest_with_kernel(
            kernel_with(
                base.kernel(),
                base.kernel().executable_digest(),
                base.kernel().code_object_digest(),
                abi,
                base.kernel().launch().clone(),
            ),
            base.manifest.target().clone(),
            base.manifest.code_objects().to_vec(),
            &changed.compiler,
            &changed.producer,
        );
        assert_axis_mismatch(&expected, &changed.binding(), "ABI");
    }

    for launch in launch_mutations(base.kernel().launch()) {
        let mut changed = base.clone();
        changed.manifest = manifest_with_kernel(
            kernel_with(
                base.kernel(),
                base.kernel().executable_digest(),
                base.kernel().code_object_digest(),
                base.kernel().abi().clone(),
                launch,
            ),
            base.manifest.target().clone(),
            base.manifest.code_objects().to_vec(),
            &changed.compiler,
            &changed.producer,
        );
        assert_axis_mismatch(&expected, &changed.binding(), "launch contract");
    }
}

#[test]
fn every_tool_measurement_and_policy_mutation_fails_closed() {
    let base = Fixture::base();
    let expected = base.binding();

    for compiler in tool_mutations(&base.compiler, "rustc-mutated") {
        let mut changed = base.clone();
        changed.compiler = compiler;
        changed.manifest = manifest_with_kernel(
            base.kernel().clone(),
            base.manifest.target().clone(),
            base.manifest.code_objects().to_vec(),
            &changed.compiler,
            &changed.producer,
        );
        assert_axis_mismatch(&expected, &changed.binding(), "compiler identity");
    }

    for producer in tool_mutations(&base.producer, "producer-mutated") {
        let mut changed = base.clone();
        changed.producer = producer;
        changed.manifest = manifest_with_kernel(
            base.kernel().clone(),
            base.manifest.target().clone(),
            base.manifest.code_objects().to_vec(),
            &changed.compiler,
            &changed.producer,
        );
        assert_axis_mismatch(&expected, &changed.binding(), "artifact-producer identity");
    }

    for execution in execution_mutations(&base.execution) {
        let mut changed = base.clone();
        changed.execution = execution;
        assert_axis_mismatch(&expected, &changed.binding(), "proof-tool identity");
    }

    let mut changed = base.clone();
    changed.configuration = vec![ConfigurationEntry::new(
        name("feature_checked"),
        text("off"),
    )];
    assert_axis_mismatch(&expected, &changed.binding(), "proof-policy identity");

    let mut changed = base.clone();
    changed.trusted_items = vec![TrustedItem::new(name("reviewed_escape"), sha(0xb0))];
    assert_axis_mismatch(&expected, &changed.binding(), "proof-policy identity");
}

#[test]
fn binding_rejects_unmatched_finalized_digest_and_non_native_payloads() {
    let base = Fixture::base();
    assert_eq!(
        base.matched().bind_finalized_executable_v1(
            &base.manifest,
            sha(0xee),
            base.code_object_version,
            &base.compiler,
            &base.producer,
            DigestAlgorithm::Sha256,
        ),
        Err(ProofExecutableBindingError::ProofTarget(
            fe2o3_artifacts::ProofTargetError::ManifestDigestMismatch("code object")
        ))
    );

    let object = CodeObjectIdentity::new(
        base.manifest.code_objects()[0].digest(),
        CodeObjectFormat::RelocatableObject,
        base.manifest.code_objects()[0].byte_len(),
    )
    .unwrap();
    let relocatable = manifest_with_kernel(
        base.kernel().clone(),
        base.manifest.target().clone(),
        vec![object],
        &base.compiler,
        &base.producer,
    );
    assert_eq!(
        base.matched().bind_finalized_executable_v1(
            &relocatable,
            base.finalized_digest(),
            base.code_object_version,
            &base.compiler,
            &base.producer,
            DigestAlgorithm::Sha256,
        ),
        Err(ProofExecutableBindingError::NonNativeCodeObject(
            CodeObjectFormat::RelocatableObject
        ))
    );
}

fn assert_axis_mismatch(
    expected: &ProofExecutableBindingV1,
    actual: &ProofExecutableBindingV1,
    field: &'static str,
) {
    assert_ne!(expected.binding_identity(), actual.binding_identity());
    assert_eq!(
        expected.validate_against(actual),
        Err(ProofExecutableBindingError::IdentityMismatch(field))
    );
}

fn manifest_with_kernel(
    kernel: KernelEntry,
    target: TargetIdentity,
    code_objects: Vec<CodeObjectIdentity>,
    compiler: &MeasuredToolIdentity,
    producer: &MeasuredToolIdentity,
) -> ManifestV1 {
    ManifestV1::new(
        CompilerIdentity::new(compiler.name().clone(), compiler.version().clone()),
        ToolIdentity::new(producer.name().clone(), producer.version().clone()),
        target,
        code_objects,
        vec![kernel],
    )
    .unwrap()
}

fn kernel_with(
    base: &KernelEntry,
    executable_digest: fe2o3_artifacts::DigestBytes,
    code_object_digest: fe2o3_artifacts::DigestBytes,
    abi: AbiLayout,
    launch: LaunchContract,
) -> KernelEntry {
    KernelEntry::new(
        base.kernel_id(),
        base.name().clone(),
        base.symbol().clone(),
        base.source_digest(),
        executable_digest,
        code_object_digest,
        base.required_capabilities().to_vec(),
        launch,
        abi,
    )
    .unwrap()
}

fn target_mutations(base: &TargetIdentity) -> Vec<TargetIdentity> {
    vec![
        TargetIdentity::new(
            text("amdgcn-amd-amdhsa-mutated"),
            base.architecture().clone(),
            base.pointer_width(),
            base.endianness(),
            base.capabilities().to_vec(),
        )
        .unwrap(),
        TargetIdentity::new(
            base.triple().clone(),
            text("gfx942:xnack-"),
            base.pointer_width(),
            base.endianness(),
            base.capabilities().to_vec(),
        )
        .unwrap(),
        TargetIdentity::new(
            base.triple().clone(),
            base.architecture().clone(),
            base.pointer_width(),
            Endianness::Big,
            base.capabilities().to_vec(),
        )
        .unwrap(),
        TargetIdentity::new(
            base.triple().clone(),
            base.architecture().clone(),
            base.pointer_width(),
            base.endianness(),
            vec![Capability::AmdWave],
        )
        .unwrap(),
    ]
}

fn abi_mutations(base: &AbiLayout) -> Vec<AbiLayout> {
    let mut changed_identity_fields = base.fields().to_vec();
    let first = &base.fields()[0];
    changed_identity_fields[0] = AbiField::new(
        first.name().clone(),
        first.offset(),
        first.size(),
        first.alignment(),
        first.kind(),
        first.mutability(),
        first.access(),
        first.address_space(),
        type_identity(0xd0, 0xd1),
        first.ownership(),
        first.alias_class(),
    )
    .unwrap();

    vec![
        AbiLayout::new(
            base.size() + 8,
            base.alignment(),
            base.pointer_width(),
            base.fields().to_vec(),
        )
        .unwrap(),
        AbiLayout::new(
            base.size(),
            base.alignment(),
            base.pointer_width(),
            changed_identity_fields,
        )
        .unwrap(),
    ]
}

fn launch_mutations(base: &LaunchContract) -> Vec<LaunchContract> {
    vec![
        LaunchContract::new(
            base.rank(),
            BlockSize::AtMost(Dimensions::new(256, 1, 1).unwrap()),
            base.max_grid(),
            base.static_shared_memory_bytes(),
            base.max_dynamic_shared_memory_bytes(),
        )
        .unwrap(),
        LaunchContract::new(
            base.rank(),
            base.block_size(),
            Dimensions::new(base.max_grid().x() - 1, 1, 1).unwrap(),
            base.static_shared_memory_bytes(),
            base.max_dynamic_shared_memory_bytes(),
        )
        .unwrap(),
        LaunchContract::new(
            base.rank(),
            base.block_size(),
            base.max_grid(),
            base.static_shared_memory_bytes(),
            base.max_dynamic_shared_memory_bytes() + 1,
        )
        .unwrap(),
    ]
}

fn tool_mutations(
    base: &MeasuredToolIdentity,
    replacement_name: &str,
) -> Vec<MeasuredToolIdentity> {
    vec![
        MeasuredToolIdentity::new(
            text(replacement_name),
            base.version().clone(),
            base.executable_digest(),
            base.configuration_digest(),
        ),
        MeasuredToolIdentity::new(
            base.name().clone(),
            text("mutated-version"),
            base.executable_digest(),
            base.configuration_digest(),
        ),
        MeasuredToolIdentity::new(
            base.name().clone(),
            base.version().clone(),
            sha(0xe0),
            base.configuration_digest(),
        ),
        MeasuredToolIdentity::new(
            base.name().clone(),
            base.version().clone(),
            base.executable_digest(),
            sha(0xe1),
        ),
    ]
}

fn execution_mutations(base: &ProofExecutionIdentity) -> Vec<ProofExecutionIdentity> {
    let mut mutations = vec![
        ProofExecutionIdentity::new(
            VerificationModelIdentity::new(text("mutated-model"), base.model().axioms_digest()),
            base.verifier().clone(),
            base.solver().clone(),
            base.evidence_recorder().clone(),
            base.invocation_digest(),
        ),
        ProofExecutionIdentity::new(
            VerificationModelIdentity::new(base.model().version().clone(), sha(0xe2)),
            base.verifier().clone(),
            base.solver().clone(),
            base.evidence_recorder().clone(),
            base.invocation_digest(),
        ),
        ProofExecutionIdentity::new(
            base.model().clone(),
            base.verifier().clone(),
            base.solver().clone(),
            base.evidence_recorder().clone(),
            sha(0xe3),
        ),
    ];

    for (slot, tool) in [
        (0, base.verifier()),
        (1, base.solver()),
        (2, base.evidence_recorder()),
    ] {
        for changed_tool in tool_mutations(tool, "mutated-proof-tool") {
            mutations.push(ProofExecutionIdentity::new(
                base.model().clone(),
                if slot == 0 {
                    changed_tool.clone()
                } else {
                    base.verifier().clone()
                },
                if slot == 1 {
                    changed_tool.clone()
                } else {
                    base.solver().clone()
                },
                if slot == 2 {
                    changed_tool
                } else {
                    base.evidence_recorder().clone()
                },
                base.invocation_digest(),
            ));
        }
    }
    mutations
}
