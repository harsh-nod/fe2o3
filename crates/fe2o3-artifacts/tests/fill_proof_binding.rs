#[allow(dead_code)]
mod common;

use common::{name, text, type_identity};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    CodeObjectFormat, CodeObjectIdentity, CompilerIdentity, ConfigurationEntry, DigestAlgorithm,
    Dimensions, Endianness, KernelEntry, LaunchContract, ManifestV1, MatchedProofEvidenceV1,
    MeasuredToolIdentity, Mutability, PayloadDigest, PointerWidth, ProofArtifactIdentity,
    ProofExecutionIdentity, ProofMatchError, ProofMatchPolicy, ProofOutcome, ProofRecordV1,
    ProofTargetIdentity, ScalarType, SourceContractIdentity, TargetIdentity, ToolIdentity,
    TrustedItem, V1_REQUIRED_PROPERTIES, VerificationModelIdentity,
};

const FILL_RUST: &[u8] = include_bytes!("../../../examples/verus_vecadd/src/lib.rs");
const FILL_PROOF: &[u8] = include_bytes!("../../../examples/verus_vecadd/verus/fill.rs");
const PROOF_RUNNER: &[u8] = include_bytes!("../../../examples/verus_vecadd/run-verus.sh");

fn identity(domain: &[u8], payload: &[u8]) -> PayloadDigest {
    let mut preimage = Vec::with_capacity(24 + domain.len() + payload.len());
    preimage.extend_from_slice(b"FE2O3-FILL-TEST\0");
    preimage.extend_from_slice(&(domain.len() as u64).to_le_bytes());
    preimage.extend_from_slice(domain);
    preimage.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    preimage.extend_from_slice(payload);
    DigestAlgorithm::Sha256.calculate(&preimage)
}

fn measured_tool(name_value: &str, version: &str, payload: &[u8]) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(
        text(name_value),
        text(version),
        identity(b"tool-executable", payload),
        identity(b"tool-configuration", name_value.as_bytes()),
    )
}

fn compiler(payload: &[u8]) -> MeasuredToolIdentity {
    measured_tool("rustc", "1.94.0", payload)
}

fn producer(payload: &[u8]) -> MeasuredToolIdentity {
    measured_tool("fe2o3", "0.1.0", payload)
}

fn execution() -> ProofExecutionIdentity {
    ProofExecutionIdentity::new(
        VerificationModelIdentity::new(
            text("fe2o3-fill-model-v1"),
            identity(b"verification-model", FILL_PROOF),
        ),
        measured_tool("verus", "0.2026.08.02.b677dd5", b"verus-fixture"),
        measured_tool("z3", "4.15.2", b"z3-fixture"),
        measured_tool("fe2o3-proof-driver", "0.1.0", PROOF_RUNNER),
        identity(b"proof-invocation", PROOF_RUNNER),
    )
}

fn source_contracts() -> SourceContractIdentity {
    SourceContractIdentity::new(
        identity(b"memory-contract", b"one exclusive u32 output slice"),
        identity(
            b"effect-contract",
            b"one identity-indexed write per active thread",
        ),
        identity(
            b"type-layout-contract",
            b"u32; slice=(ptr,len); 64-bit pointers",
        ),
        identity(
            b"hardware-thread-id-contract",
            b"active_slot < thread_count && observed_id == active_slot",
        ),
        identity(b"functional-fill-contract", FILL_PROOF),
    )
}

fn fill_abi() -> AbiLayout {
    AbiLayout::new(
        24,
        8,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                name("output"),
                0,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                Mutability::Mutable,
                Access::WriteOnly,
                AddressSpace::Global,
                type_identity(0xa0, 0xa1),
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
            .unwrap(),
            AbiField::new(
                name("value"),
                16,
                4,
                4,
                AbiKind::Scalar(ScalarType::U32),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                type_identity(0xb0, 0xb1),
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn source_tree_digest() -> PayloadDigest {
    let mut sources = Vec::with_capacity(FILL_RUST.len() + FILL_PROOF.len());
    sources.extend_from_slice(FILL_RUST);
    sources.extend_from_slice(FILL_PROOF);
    identity(b"source-tree", &sources)
}

fn executable_digest() -> PayloadDigest {
    identity(
        b"executable-semantics",
        b"fill(output: &mut [u32], value: u32)",
    )
}

fn code_object_digest() -> PayloadDigest {
    identity(b"code-object", b"synthetic fill proof-binding fixture")
}

fn kernel_id() -> PayloadDigest {
    identity(b"kernel-id", b"verus_fill_identity_write_v1")
}

fn manifest() -> ManifestV1 {
    let code_object = code_object_digest();
    let kernel = KernelEntry::new(
        kernel_id().bytes(),
        name("verus_fill"),
        name("verus_fill.kd"),
        source_tree_digest().bytes(),
        executable_digest().bytes(),
        code_object.bytes(),
        vec![],
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
            Dimensions::new(65_535, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap(),
        fill_abi(),
    )
    .unwrap();

    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        TargetIdentity::new(
            text("amdgcn-amd-amdhsa"),
            text("gfx1151"),
            PointerWidth::Bits64,
            Endianness::Little,
            vec![],
        )
        .unwrap(),
        vec![
            CodeObjectIdentity::new(
                code_object.bytes(),
                CodeObjectFormat::NativeExecutable,
                4096,
            )
            .unwrap(),
        ],
        vec![kernel],
    )
    .unwrap()
}

fn target_with_tools(
    compiler: &MeasuredToolIdentity,
    producer: &MeasuredToolIdentity,
) -> ProofTargetIdentity {
    manifest()
        .proof_target(
            kernel_id(),
            identity(b"kernel-instance", b"verus_fill::<u32>"),
            source_tree_digest(),
            identity(b"crate-graph", b"verus-vecadd + fe2o3-contracts"),
            executable_digest(),
            code_object_digest(),
            source_contracts(),
            compiler,
            producer,
            DigestAlgorithm::Sha256,
        )
        .unwrap()
}

fn target() -> ProofTargetIdentity {
    target_with_tools(&compiler(b"rustc-fixture"), &producer(b"fe2o3-fixture"))
}

fn configuration() -> Vec<ConfigurationEntry> {
    vec![
        ConfigurationEntry::new(name("address_space_bits"), text("64")),
        ConfigurationEntry::new(
            name("launch_mapping"),
            text("global_id_x_equals_active_slot"),
        ),
    ]
}

fn trusted_items() -> Vec<TrustedItem> {
    vec![TrustedItem::new(
        name("hardware_thread_id_contract"),
        source_contracts().capability_semantics_digest(),
    )]
}

fn record(target: ProofTargetIdentity) -> ProofRecordV1 {
    // This is deliberately synthetic self-reported evidence. The test exercises
    // exact matching; it does not claim these properties were derived by a driver.
    ProofRecordV1::new(
        target,
        configuration(),
        execution(),
        ProofOutcome::Proved,
        V1_REQUIRED_PROPERTIES.to_vec(),
        trusted_items(),
    )
    .unwrap()
}

fn policy() -> ProofMatchPolicy {
    ProofMatchPolicy::new(target(), configuration(), execution(), trusted_items()).unwrap()
}

#[test]
fn exact_fill_evidence_matches_without_claiming_compiler_refinement() {
    let record = record(target());
    let expected_digest = record.digest(DigestAlgorithm::Sha256);

    let matched: MatchedProofEvidenceV1 = policy()
        .match_record(record, DigestAlgorithm::Sha256)
        .unwrap();

    assert_eq!(matched.record_digest(), expected_digest);
    assert_eq!(matched.record().target(), target());
}

#[test]
fn fill_kernel_and_every_source_contract_identity_are_exact() {
    let expected = target();
    let artifact = expected.artifact();
    let contracts = expected.source_contracts();
    let stale_kernel = ProofTargetIdentity::new(
        ProofArtifactIdentity::new(
            identity(b"kernel-id", b"different-fill"),
            artifact.instance_digest(),
            artifact.source_tree_digest(),
            artifact.crate_graph_digest(),
            artifact.executable_digest(),
            artifact.environment_digest(),
            artifact.artifact_selection_digest(),
            artifact.artifact_contract_digest(),
        ),
        contracts,
    );
    assert_eq!(
        policy().match_record(record(stale_kernel), DigestAlgorithm::Sha256),
        Err(ProofMatchError::IdentityMismatch("kernel"))
    );

    let fields = [
        "memory contract",
        "effect contract",
        "type layout contract",
        "capability semantics contract",
        "functional specification contract",
    ];
    for (index, field) in fields.into_iter().enumerate() {
        let mut values = [
            contracts.memory_digest(),
            contracts.effects_digest(),
            contracts.type_layout_digest(),
            contracts.capability_semantics_digest(),
            contracts.functional_specification_digest(),
        ];
        values[index] = identity(field.as_bytes(), b"stale");
        let stale = ProofTargetIdentity::new(
            artifact,
            SourceContractIdentity::new(values[0], values[1], values[2], values[3], values[4]),
        );
        assert_eq!(
            policy().match_record(record(stale), DigestAlgorithm::Sha256),
            Err(ProofMatchError::IdentityMismatch(field))
        );
    }
}

#[test]
fn fill_environment_binds_each_measured_compiler_and_producer_input() {
    let expected = target();
    let variants = [
        target_with_tools(&compiler(b"different-rustc"), &producer(b"fe2o3-fixture")),
        target_with_tools(&compiler(b"rustc-fixture"), &producer(b"different-fe2o3")),
    ];

    for stale in variants {
        assert_eq!(
            stale.artifact().kernel_id(),
            expected.artifact().kernel_id()
        );
        assert_ne!(
            stale.artifact().environment_digest(),
            expected.artifact().environment_digest()
        );
        assert_eq!(
            policy().match_record(record(stale), DigestAlgorithm::Sha256),
            Err(ProofMatchError::IdentityMismatch(
                "compiler and target environment"
            ))
        );
    }
}
