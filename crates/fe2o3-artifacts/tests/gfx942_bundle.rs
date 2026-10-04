#[allow(dead_code)]
mod common;

use common::{digest, kernel_with_object_digest, name, text};
use fe2o3_artifacts::{
    AbiLayout, ArtifactContainerV1, BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity,
    CodeObjectPayload, CompilerIdentity, ConfigurationEntry, ContainerDecodeError, DigestAlgorithm,
    Dimensions, Endianness, ExecutableCodeObjectVersionV1, GFX942_TWO_KERNEL_BUNDLE_VERSION_V1,
    Gfx942BundleError, Gfx942KernelProofBindingV1, Gfx942TwoKernelBundleV1, KernelEntry,
    LaunchContract, ManifestV1, MeasuredToolIdentity, PayloadDigest, PointerWidth,
    ProofExecutionIdentity, ProofMatchPolicy, ProofOutcome, ProofRecordV1, SourceContractIdentity,
    TargetIdentity, ToolIdentity, V1_REQUIRED_PROPERTIES, ValidationError,
    VerificationModelIdentity,
};

const PAYLOAD_BYTES: &[u8] = b"native gfx942 payload containing alpha.kd and beta.kd";

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

fn gfx942_target() -> TargetIdentity {
    TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx942"),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::AmdWave],
    )
    .unwrap()
}

fn payload() -> CodeObjectPayload {
    CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, PAYLOAD_BYTES.to_vec()).unwrap()
}

fn payload_digest() -> fe2o3_artifacts::DigestBytes {
    DigestAlgorithm::Sha256.calculate(PAYLOAD_BYTES).bytes()
}

fn launch(block_x: u32, dynamic_shared_memory: u32) -> LaunchContract {
    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(block_x, 1, 1).unwrap()),
        Dimensions::new(65_535, 1, 1).unwrap(),
        0,
        dynamic_shared_memory,
    )
    .unwrap()
}

fn kernels() -> [KernelEntry; 2] {
    let prototype = kernel_with_object_digest(
        0x11,
        "alpha",
        "alpha.kd",
        payload_digest(),
        vec![Capability::AmdWave],
    );
    let abi = prototype.abi().clone();
    [
        KernelEntry::new(
            digest(0x11),
            name("alpha"),
            name("alpha.kd"),
            digest(0x21),
            digest(0x31),
            payload_digest(),
            vec![Capability::AmdWave],
            launch(256, 0),
            abi.clone(),
        )
        .unwrap(),
        KernelEntry::new(
            digest(0x12),
            name("beta"),
            name("beta.kd"),
            digest(0x22),
            digest(0x32),
            payload_digest(),
            vec![Capability::AmdWave],
            launch(128, 2048),
            abi,
        )
        .unwrap(),
    ]
}

fn contracts(slot: usize) -> SourceContractIdentity {
    let base = 0x80 + (slot as u8 * 8);
    SourceContractIdentity::new(
        sha(base),
        sha(base + 1),
        sha(base + 2),
        sha(base + 3),
        sha(base + 4),
    )
}

fn compiler_identity() -> CompilerIdentity {
    let compiler = compiler();
    CompilerIdentity::new(compiler.name().clone(), compiler.version().clone())
}

fn producer_identity() -> ToolIdentity {
    let producer = producer();
    ToolIdentity::new(producer.name().clone(), producer.version().clone())
}

fn manifest_for(kernels: [KernelEntry; 2]) -> ManifestV1 {
    ManifestV1::new(
        compiler_identity(),
        producer_identity(),
        gfx942_target(),
        vec![
            CodeObjectIdentity::new(
                payload_digest(),
                CodeObjectFormat::NativeExecutable,
                PAYLOAD_BYTES.len() as u64,
            )
            .unwrap(),
        ],
        Vec::from(kernels),
    )
    .unwrap()
}

fn proof_binding_for(
    manifest: &ManifestV1,
    kernel_id: fe2o3_artifacts::DigestBytes,
    source_contracts: SourceContractIdentity,
) -> fe2o3_artifacts::ProofExecutableBindingV1 {
    let kernel = manifest
        .kernels()
        .iter()
        .find(|kernel| kernel.kernel_id() == kernel_id)
        .unwrap();
    let compiler = compiler();
    let producer = producer();
    let target = manifest
        .proof_target(
            tagged(kernel.kernel_id()),
            sha(kernel_id.as_bytes()[0].wrapping_add(0x30)),
            tagged(kernel.source_digest()),
            sha(kernel_id.as_bytes()[0].wrapping_add(0x40)),
            tagged(kernel.executable_digest()),
            tagged(payload_digest()),
            source_contracts,
            &compiler,
            &producer,
            DigestAlgorithm::Sha256,
        )
        .unwrap();
    let configuration = vec![ConfigurationEntry::new(name("proof_profile"), text("v1"))];
    let execution = execution();
    let record = ProofRecordV1::new(
        target,
        configuration.clone(),
        execution.clone(),
        ProofOutcome::Proved,
        V1_REQUIRED_PROPERTIES.to_vec(),
        vec![],
    )
    .unwrap();
    ProofMatchPolicy::new(target, configuration, execution, vec![])
        .unwrap()
        .match_record(record, DigestAlgorithm::Sha256)
        .unwrap()
        .bind_finalized_executable_v1(
            manifest,
            tagged(payload_digest()),
            ExecutableCodeObjectVersionV1::V5,
            &compiler,
            &producer,
            DigestAlgorithm::Sha256,
        )
        .unwrap()
}

fn proof_bindings_for(manifest: &ManifestV1) -> [Gfx942KernelProofBindingV1; 2] {
    let first = &manifest.kernels()[0];
    let second = &manifest.kernels()[1];
    [
        Gfx942KernelProofBindingV1::new(
            first.kernel_id(),
            contracts(0).effects_digest(),
            proof_binding_for(manifest, first.kernel_id(), contracts(0)),
        ),
        Gfx942KernelProofBindingV1::new(
            second.kernel_id(),
            contracts(1).effects_digest(),
            proof_binding_for(manifest, second.kernel_id(), contracts(1)),
        ),
    ]
}

fn bundle_with_order(reversed: bool) -> Gfx942TwoKernelBundleV1 {
    let mut kernels = kernels();
    let manifest = manifest_for(kernels.clone());
    let mut proofs = proof_bindings_for(&manifest);
    if reversed {
        kernels.swap(0, 1);
        proofs.swap(0, 1);
    }
    Gfx942TwoKernelBundleV1::build(
        compiler_identity(),
        producer_identity(),
        gfx942_target(),
        kernels,
        payload(),
        proofs,
    )
    .unwrap()
}

#[test]
fn canonical_two_kernel_bundle_round_trips_through_existing_container_wire() {
    let bundle = bundle_with_order(false);
    let bytes = bundle.to_container_bytes();
    let decoded =
        Gfx942TwoKernelBundleV1::from_container_bytes(&bytes, bundle.proof_bindings().clone())
            .unwrap();

    assert_eq!(bundle.version(), GFX942_TWO_KERNEL_BUNDLE_VERSION_V1);
    assert_eq!(decoded, bundle);
    assert_eq!(decoded.to_container_bytes(), bytes);
    assert_eq!(decoded.container().manifest().kernels().len(), 2);
    assert_eq!(decoded.container().payloads().len(), 1);
    assert_eq!(decoded.index().payloads().len(), 1);
    assert!(
        decoded
            .index()
            .kernels()
            .iter()
            .all(|kernel| { kernel.payload_digests() == [decoded.index().payloads()[0].digest()] })
    );
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
}

#[test]
fn reordered_kernel_and_proof_inputs_have_identical_canonical_outputs() {
    let ordered = bundle_with_order(false);
    let reversed = bundle_with_order(true);

    assert_eq!(ordered.to_container_bytes(), reversed.to_container_bytes());
    assert_eq!(ordered.index().to_bytes(), reversed.index().to_bytes());
    assert_eq!(ordered.proof_bindings(), reversed.proof_bindings());
    assert!(
        ordered
            .container()
            .manifest()
            .kernels()
            .windows(2)
            .all(|pair| pair[0].kernel_id() < pair[1].kernel_id())
    );
}

#[test]
fn malformed_wire_and_wrong_profile_shape_are_rejected() {
    let bundle = bundle_with_order(false);
    let proofs = bundle.proof_bindings().clone();
    let mut malformed = bundle.to_container_bytes();
    malformed[0] ^= 0xff;
    assert_eq!(
        Gfx942TwoKernelBundleV1::from_container_bytes(&malformed, proofs.clone()),
        Err(Gfx942BundleError::Decode(
            ContainerDecodeError::InvalidMagic
        ))
    );

    let manifest = manifest_for(kernels());
    let single_kernel_manifest = ManifestV1::new(
        manifest.compiler().clone(),
        manifest.producer().clone(),
        manifest.target().clone(),
        manifest.code_objects().to_vec(),
        vec![manifest.kernels()[0].clone()],
    )
    .unwrap();
    let single_kernel = ArtifactContainerV1::new(
        single_kernel_manifest,
        DigestAlgorithm::Sha256,
        vec![payload()],
    )
    .unwrap();
    assert_eq!(
        Gfx942TwoKernelBundleV1::from_container(single_kernel, proofs),
        Err(Gfx942BundleError::UnexpectedCount {
            field: "kernels",
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn duplicate_names_proofs_and_conflicting_payloads_are_rejected() {
    let valid_kernels = kernels();
    let manifest = manifest_for(valid_kernels.clone());
    let proofs = proof_bindings_for(&manifest);
    let duplicate_name = KernelEntry::new(
        valid_kernels[1].kernel_id(),
        valid_kernels[0].name().clone(),
        valid_kernels[1].symbol().clone(),
        valid_kernels[1].source_digest(),
        valid_kernels[1].executable_digest(),
        valid_kernels[1].code_object_digest(),
        valid_kernels[1].required_capabilities().to_vec(),
        valid_kernels[1].launch().clone(),
        valid_kernels[1].abi().clone(),
    )
    .unwrap();
    assert_eq!(
        Gfx942TwoKernelBundleV1::build(
            compiler_identity(),
            producer_identity(),
            gfx942_target(),
            [valid_kernels[0].clone(), duplicate_name],
            payload(),
            proofs.clone(),
        ),
        Err(Gfx942BundleError::Model(ValidationError::Duplicate {
            field: "kernel name"
        }))
    );

    let duplicate_proof = proofs[0].clone();
    assert_eq!(
        Gfx942TwoKernelBundleV1::build(
            compiler_identity(),
            producer_identity(),
            gfx942_target(),
            valid_kernels.clone(),
            payload(),
            [proofs[0].clone(), duplicate_proof],
        ),
        Err(Gfx942BundleError::DuplicateProofKernel(
            valid_kernels[0].kernel_id()
        ))
    );

    let conflicting = KernelEntry::new(
        valid_kernels[1].kernel_id(),
        valid_kernels[1].name().clone(),
        valid_kernels[1].symbol().clone(),
        valid_kernels[1].source_digest(),
        valid_kernels[1].executable_digest(),
        digest(0xee),
        valid_kernels[1].required_capabilities().to_vec(),
        valid_kernels[1].launch().clone(),
        valid_kernels[1].abi().clone(),
    )
    .unwrap();
    assert_eq!(
        Gfx942TwoKernelBundleV1::build(
            compiler_identity(),
            producer_identity(),
            gfx942_target(),
            [valid_kernels[0].clone(), conflicting],
            payload(),
            proofs,
        ),
        Err(Gfx942BundleError::ConflictingPayloadIdentity {
            kernel_id: valid_kernels[1].kernel_id()
        })
    );
}

#[test]
fn cross_kernel_and_effect_proof_substitutions_are_rejected() {
    let valid_kernels = kernels();
    let manifest = manifest_for(valid_kernels.clone());
    let proofs = proof_bindings_for(&manifest);
    let crossed = [
        Gfx942KernelProofBindingV1::new(
            valid_kernels[0].kernel_id(),
            contracts(1).effects_digest(),
            proofs[1].binding().clone(),
        ),
        Gfx942KernelProofBindingV1::new(
            valid_kernels[1].kernel_id(),
            contracts(0).effects_digest(),
            proofs[0].binding().clone(),
        ),
    ];
    assert_eq!(
        Gfx942TwoKernelBundleV1::build(
            compiler_identity(),
            producer_identity(),
            gfx942_target(),
            valid_kernels.clone(),
            payload(),
            crossed,
        ),
        Err(Gfx942BundleError::CrossKernelProofSubstitution {
            declared: valid_kernels[0].kernel_id(),
            bound: valid_kernels[1].kernel_id(),
        })
    );

    let stale_effects = [
        Gfx942KernelProofBindingV1::new(
            proofs[0].kernel_id(),
            sha(0xfe),
            proofs[0].binding().clone(),
        ),
        proofs[1].clone(),
    ];
    assert_eq!(
        Gfx942TwoKernelBundleV1::build(
            compiler_identity(),
            producer_identity(),
            gfx942_target(),
            valid_kernels.clone(),
            payload(),
            stale_effects,
        ),
        Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: valid_kernels[0].kernel_id(),
            field: "effects identity",
        })
    );
}

#[test]
fn proof_abi_and_launch_must_match_their_exact_manifest_entry() {
    let valid_kernels = kernels();
    let manifest = manifest_for(valid_kernels.clone());
    let valid_proofs = proof_bindings_for(&manifest);

    let changed_abi = AbiLayout::new(
        valid_kernels[0].abi().size() + 8,
        valid_kernels[0].abi().alignment(),
        valid_kernels[0].abi().pointer_width(),
        valid_kernels[0].abi().fields().to_vec(),
    )
    .unwrap();
    let abi_kernel = replace_kernel_contract(
        &valid_kernels[0],
        changed_abi,
        valid_kernels[0].launch().clone(),
    );
    let abi_manifest = manifest_for([abi_kernel, valid_kernels[1].clone()]);
    let stale_abi_proof = Gfx942KernelProofBindingV1::new(
        valid_kernels[0].kernel_id(),
        contracts(0).effects_digest(),
        proof_binding_for(&abi_manifest, valid_kernels[0].kernel_id(), contracts(0)),
    );
    assert_eq!(
        Gfx942TwoKernelBundleV1::build(
            compiler_identity(),
            producer_identity(),
            gfx942_target(),
            valid_kernels.clone(),
            payload(),
            [stale_abi_proof, valid_proofs[1].clone()],
        ),
        Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: valid_kernels[0].kernel_id(),
            field: "ABI identity",
        })
    );

    let launch_kernel = replace_kernel_contract(
        &valid_kernels[0],
        valid_kernels[0].abi().clone(),
        launch(64, 1024),
    );
    let launch_manifest = manifest_for([launch_kernel, valid_kernels[1].clone()]);
    let stale_launch_proof = Gfx942KernelProofBindingV1::new(
        valid_kernels[0].kernel_id(),
        contracts(0).effects_digest(),
        proof_binding_for(&launch_manifest, valid_kernels[0].kernel_id(), contracts(0)),
    );
    assert_eq!(
        Gfx942TwoKernelBundleV1::build(
            compiler_identity(),
            producer_identity(),
            gfx942_target(),
            valid_kernels.clone(),
            payload(),
            [stale_launch_proof, valid_proofs[1].clone()],
        ),
        Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: valid_kernels[0].kernel_id(),
            field: "launch identity",
        })
    );
}

fn replace_kernel_contract(
    kernel: &KernelEntry,
    abi: AbiLayout,
    launch: LaunchContract,
) -> KernelEntry {
    KernelEntry::new(
        kernel.kernel_id(),
        kernel.name().clone(),
        kernel.symbol().clone(),
        kernel.source_digest(),
        kernel.executable_digest(),
        kernel.code_object_digest(),
        kernel.required_capabilities().to_vec(),
        launch,
        abi,
    )
    .unwrap()
}
