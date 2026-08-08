use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, DurableLinkPublicationPlanV1, DurablePublishedClaimCodecErrorV1,
    DurablePublishedHsacoClaimV1, PackageIdentityV1, ProducerIdentity,
    UpstreamCodeObjectEvidenceIdentityV1, begin_build_attempt, finish_build_attempt,
    publish_exact_hsaco_evidence_for_attempt_v1,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, BundleIndexV1, CallerClaimedPackageIdentityV1,
    CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload, CompilerIdentity,
    DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions,
    DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1,
    DirectLinkFfiClosureIdentityV1, DirectLinkFinalizationIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
    DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
    DirectLinkToolchainConfigurationIdentityV1, DirectLinkToolchainExecutableIdentityV1,
    DirectLinkToolchainIdentityV1, DirectLinkTransformationIdentityV1,
    DirectLinkWorkerConfigurationIdentityV1, DirectLinkWorkerExecutableIdentityV1,
    DirectLinkWorkerIdentityV1, Endianness, IdentityText, KernelEntry, LaunchContract,
    ManifestClaimDerivedLinkPublicationScopeV1, ManifestClaimDirectLinkPublicationBridgeV1,
    ManifestV1, MeasuredToolIdentity, Mutability, Name, PayloadDigest, PointerWidth,
    ProofArtifactIdentity, ProofExecutionIdentity, ProofOutcome, ProofProperty, ProofRecordV1,
    ProofTargetIdentity, ScalarType, SourceContractIdentity, TargetIdentity, ToolIdentity,
    TypeIdentity, VerificationModelIdentity,
};
use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion, CompilerIdentityV1,
    DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1,
    DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1,
    KernelId, LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1,
    SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
};
use fe2o3_worker_v2_bundle::{
    DescriptorLineageV1, EnvelopeDecodeError, EnvelopeInputsDecodeError, EnvelopeValidationError,
    ExactRawHsacoV1, MAX_WORKER_V2_RAW_HSACO_BYTES, WorkerV2EnvelopeInputsV1,
    WorkerV2LoadEnvelopeV1,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const KERNELS: [(&str, u8, u8, u8); 2] = [("alpha", 0x21, 0x31, 0x41), ("zeta", 0x22, 0x32, 0x42)];
const FINALIZED_BYTE: u8 = 0xf1;
const RAW_BYTE: u8 = 0x71;
static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v2-envelope-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn output(&self) -> PathBuf {
        self.path.join("output")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Parts {
    container: ArtifactContainerV1,
    bundle: BundleIndexV1,
    evidence: DirectLinkBundleEvidenceV1,
    descriptor: DescriptorLineageV1,
    proofs: Vec<ProofRecordV1>,
    raw: ExactRawHsacoV1,
    claim: DurablePublishedHsacoClaimV1,
}

impl Parts {
    fn into_envelope(self) -> Result<WorkerV2LoadEnvelopeV1, EnvelopeValidationError> {
        WorkerV2LoadEnvelopeV1::new(
            self.container,
            self.bundle,
            self.evidence,
            self.descriptor,
            self.proofs,
            self.raw,
            self.claim,
        )
    }
}

fn digest(seed: u8) -> DigestBytes {
    DigestBytes::from_bytes([seed; 32])
}

fn tagged(seed: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(seed))
}

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn descriptor_text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn descriptor_name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn manifest_target() -> TargetIdentity {
    TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx942:xnack-"),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![],
    )
    .unwrap()
}

fn manifest_abi() -> AbiLayout {
    AbiLayout::new(
        4,
        4,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                name("value"),
                0,
                4,
                4,
                AbiKind::Scalar(ScalarType::F32),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                TypeIdentity::new(
                    DeclaredRustTypeIdentity::from_untrusted_bytes(digest(0xa1)),
                    DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(0xa2)),
                ),
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn manifest_launch() -> LaunchContract {
    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(65_535, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap()
}

fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}

fn descriptor_launch() -> LaunchConstraintsV1 {
    LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
        DimensionsV1::new(65_535, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap()
}

fn descriptor_table() -> DeviceDescriptorTableV1 {
    let source_type = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let kernels = KERNELS
        .iter()
        .map(|(kernel_name, id, source, executable)| {
            KernelDescriptorV1::new(
                KernelId::from_bytes([*id; 32]),
                descriptor_name(kernel_name),
                descriptor_name(kernel_name),
                descriptor_name(&format!("{kernel_name}.kd")),
                evidence(id.wrapping_add(0x50), *source),
                evidence(id.wrapping_add(0x60), *executable),
                vec![],
                KernelAbiLayoutV1::new(4, 4, 4).unwrap(),
                descriptor_launch(),
                vec![
                    LogicalArgumentV1::scalar(
                        0,
                        descriptor_name("value"),
                        &source_type,
                        &layout,
                        0,
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        })
        .collect();
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0xc0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            descriptor_text("rustc"),
            descriptor_text("1.94.0-nightly"),
            [0x11; 20],
        ),
        ProducerIdentityV1::new(descriptor_text("cargo-fe2o3"), descriptor_text("0.1.0")),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        vec![source_type],
        vec![layout],
        kernels,
    )
    .unwrap()
}

fn proof_with_lineage(kernel_id: u8, source: u8, executable: u8) -> ProofRecordV1 {
    let artifact = ProofArtifactIdentity::new(
        tagged(kernel_id),
        tagged(0x51),
        tagged(source),
        tagged(0x53),
        tagged(executable),
        tagged(0x55),
        tagged(0x56),
        tagged(0x57),
    );
    let contracts = SourceContractIdentity::new(
        tagged(0x61),
        tagged(0x62),
        tagged(0x63),
        tagged(0x64),
        tagged(0x65),
    );
    let tool = |tool_name: &str, seed: u8| {
        MeasuredToolIdentity::new(
            text(tool_name),
            text("1.0"),
            tagged(seed),
            tagged(seed.wrapping_add(1)),
        )
    };
    ProofRecordV1::new(
        ProofTargetIdentity::new(artifact, contracts),
        vec![],
        ProofExecutionIdentity::new(
            VerificationModelIdentity::new(text("verus-model-v1"), tagged(0x70)),
            tool("verus", 0x71),
            tool("z3", 0x73),
            tool("recorder", 0x75),
            tagged(0x77),
        ),
        ProofOutcome::Proved,
        vec![ProofProperty::Bounds],
        vec![],
    )
    .unwrap()
}

fn proof(kernel_id: u8) -> ProofRecordV1 {
    let (_, _, source, executable) = KERNELS
        .iter()
        .find(|(_, id, _, _)| *id == kernel_id)
        .expect("known fixture kernel");
    proof_with_lineage(kernel_id, *source, *executable)
}

fn direct_link_expectation(
    raw: PayloadDigest,
    finalized: PayloadDigest,
) -> DirectLinkBindingExpectationV1 {
    DirectLinkBindingExpectationV1::new(
        DirectLinkRequestIdentityV1::new(tagged(0x81)),
        DirectLinkWorkerIdentityV1::new(
            text("fe2o3-worker"),
            text("v2"),
            DirectLinkWorkerExecutableIdentityV1::new(tagged(0x82)),
            DirectLinkWorkerConfigurationIdentityV1::new(tagged(0x83)),
        ),
        DirectLinkToolchainIdentityV1::new(
            text("llvm"),
            text("22"),
            DirectLinkToolchainExecutableIdentityV1::new(tagged(0x84)),
            DirectLinkToolchainConfigurationIdentityV1::new(tagged(0x85)),
        ),
        DirectLinkResponseIdentityV1::new(tagged(0x86)),
        DirectLinkTransformationIdentityV1::new(
            DirectLinkLinkedOutputIdentityV1::new(raw),
            DirectLinkFinalizationIdentityV1::new(tagged(0x87)),
            DirectLinkFinalizedPayloadIdentityV1::new(finalized),
        ),
        DirectLinkFfiClosureIdentityV1::new(tagged(0x88)),
    )
}

fn build_parts() -> Parts {
    build_parts_with_upstream(None)
}

fn build_parts_with_upstream(upstream_override: Option<[u8; 32]>) -> Parts {
    let finalized_bytes = vec![FINALIZED_BYTE; 64];
    let raw_bytes = vec![RAW_BYTE; 64];
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, finalized_bytes.clone()).unwrap();
    let finalized_identity = payload.digest();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0-nightly")),
        ToolIdentity::new(text("cargo-fe2o3"), text("0.1.0")),
        manifest_target(),
        vec![
            CodeObjectIdentity::new(
                finalized_identity.bytes(),
                CodeObjectFormat::NativeExecutable,
                payload.bytes().len() as u64,
            )
            .unwrap(),
        ],
        KERNELS
            .iter()
            .map(|(kernel_name, id, source, executable)| {
                KernelEntry::new(
                    digest(*id),
                    name(kernel_name),
                    name(kernel_name),
                    digest(*source),
                    digest(*executable),
                    finalized_identity.bytes(),
                    vec![],
                    manifest_launch(),
                    manifest_abi(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let container =
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
    let raw = ExactRawHsacoV1::from_bytes(raw_bytes).unwrap();
    let expectation = direct_link_expectation(raw.identity(), finalized_identity);
    let evidence = DirectLinkBundleEvidenceV1::bind(
        &bundle,
        &[&container],
        &[DirectLinkBindingSourceV1::new(
            &container,
            expectation.clone(),
        )],
    )
    .unwrap();
    let validated = evidence
        .validate_against(
            &bundle,
            &[&container],
            &[DirectLinkBindingSourceV1::new(&container, expectation)],
        )
        .unwrap();
    let publication_dir = TestDirectory::new();
    let output = publication_dir.output();
    let owner = ProducerIdentity::from_codegen(
        "fe2o3_worker_v2_bundle_test",
        Some(Path::new("/src/worker-v2-bundle-test.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &output,
        &owner,
        BuildInvocation::from_bytes([0x43; 32]),
        BuildSession::from_bytes([0x42; 16]),
    )
    .unwrap();
    let publication_scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
        CallerClaimedPackageIdentityV1::new(PackageIdentityV1::from_bytes([0x91; 32])),
        &validated,
        0,
        &container,
    )
    .unwrap();
    let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
        attempt,
        publication_scope,
        &validated,
        0,
    )
    .unwrap();
    let scope = bridge
        .non_authoritative_diagnostics()
        .descriptive_scope_claim();
    let plan = DurableLinkPublicationPlanV1::new(
        attempt,
        scope,
        bridge.request_identity(),
        bridge.worker_identity(),
        bridge.response_identity(),
        bridge.linked_output_identity(),
        bridge.finalization_identity(),
        bridge.finalized_output_identity(),
        bridge.publication_identity(),
    );
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
        upstream_override.unwrap_or_else(|| Sha256::digest(evidence.to_bytes()).into()),
    );
    let publication = publish_exact_hsaco_evidence_for_attempt_v1(
        &output,
        &owner,
        attempt,
        plan,
        upstream,
        &finalized_bytes,
    )
    .unwrap();
    let claim = publication.published_claim().clone();
    drop(publication);
    finish_build_attempt(&output, &owner, attempt).unwrap();
    Parts {
        container,
        bundle,
        evidence,
        descriptor: DescriptorLineageV1::new(descriptor_table()),
        proofs: vec![proof(0x22), proof(0x21)],
        raw,
        claim,
    }
}

fn component_offsets(bytes: &[u8]) -> (usize, usize, usize) {
    let read_u32 =
        |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    let container_len = read_u32(16);
    let bundle_len = read_u32(20);
    let direct_len = read_u32(24);
    let descriptor_len = read_u32(28);
    let publication_len = u16::from_le_bytes(bytes[40..42].try_into().unwrap()) as usize;
    let proof_start =
        77 + container_len + bundle_len + direct_len + descriptor_len + publication_len;
    let raw_len = read_u32(32);
    let raw_start = bytes.len() - raw_len;
    (77, proof_start, raw_start)
}

#[test]
fn deterministic_round_trip_is_canonical() {
    let envelope = build_parts().into_envelope().unwrap();
    let bytes = envelope.to_bytes();
    let decoded = WorkerV2LoadEnvelopeV1::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.to_bytes(), bytes);
    assert_eq!(
        decoded.proof_records()[0].target().artifact().kernel_id(),
        tagged(0x21)
    );
    assert_eq!(
        decoded.proof_records()[1].target().artifact().kernel_id(),
        tagged(0x22)
    );
    assert_eq!(decoded.finalized_payload(), vec![FINALIZED_BYTE; 64]);
    assert!(!decoded.grants_currentness_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert_eq!(decoded.published_claim(), envelope.published_claim());
}

#[test]
fn every_truncation_and_trailing_bytes_fail_closed() {
    let bytes = build_parts().into_envelope().unwrap().to_bytes();
    for length in 0..bytes.len() {
        assert!(
            WorkerV2LoadEnvelopeV1::from_bytes(&bytes[..length]).is_err(),
            "accepted truncated length {length}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(matches!(
        WorkerV2LoadEnvelopeV1::from_bytes(&trailing),
        Err(EnvelopeDecodeError::TrailingBytes)
    ));
}

#[test]
fn duplicate_and_incomplete_proof_closures_are_rejected() {
    let mut duplicate = build_parts();
    duplicate.proofs = vec![proof(0x21), proof(0x21)];
    assert!(matches!(
        duplicate.into_envelope(),
        Err(EnvelopeValidationError::DuplicateProofKernel)
    ));

    let mut incomplete = build_parts();
    incomplete.proofs.pop();
    assert!(matches!(
        incomplete.into_envelope(),
        Err(EnvelopeValidationError::ProofCountMismatch)
    ));
}

#[test]
fn proof_source_and_executable_substitution_are_rejected() {
    let mut wrong_source = build_parts();
    wrong_source.proofs[0] = proof_with_lineage(0x22, 0xee, 0x42);
    assert!(matches!(
        wrong_source.into_envelope(),
        Err(EnvelopeValidationError::ProofManifestMismatch {
            field: "source identity"
        })
    ));

    let mut wrong_executable = build_parts();
    wrong_executable.proofs[0] = proof_with_lineage(0x22, 0x32, 0xee);
    assert!(matches!(
        wrong_executable.into_envelope(),
        Err(EnvelopeValidationError::ProofManifestMismatch {
            field: "executable identity"
        })
    ));
}

#[test]
fn incomplete_bundle_closure_is_rejected() {
    let mut parts = build_parts();
    parts.bundle = BundleIndexV1::new(
        parts.bundle.target_associations().to_vec(),
        parts.bundle.payloads().to_vec(),
        vec![parts.bundle.kernels()[0].clone()],
    )
    .unwrap();
    assert!(matches!(
        parts.into_envelope(),
        Err(EnvelopeValidationError::BundleDoesNotMatchContainer)
    ));
}

#[test]
fn wrong_raw_digest_and_raw_final_substitution_are_rejected() {
    assert!(matches!(
        ExactRawHsacoV1::new(tagged(0xaa), vec![RAW_BYTE; 64]),
        Err(EnvelopeValidationError::RawHsacoDigestMismatch)
    ));

    let mut bytes = build_parts().into_envelope().unwrap().to_bytes();
    let (container_start, _, raw_start) = component_offsets(&bytes);
    let final_start = bytes[container_start..raw_start]
        .windows(64)
        .position(|window| window == [FINALIZED_BYTE; 64])
        .map(|offset| container_start + offset)
        .unwrap();
    for index in 0..64 {
        bytes.swap(final_start + index, raw_start + index);
    }
    assert!(WorkerV2LoadEnvelopeV1::from_bytes(&bytes).is_err());
}

#[test]
fn corrupted_published_claim_is_rejected() {
    let mut bytes = build_parts().into_envelope().unwrap().to_bytes();
    let (_, proof_start, _) = component_offsets(&bytes);
    let published_claim_len = u16::from_le_bytes(bytes[40..42].try_into().unwrap()) as usize;
    let published_claim_start = proof_start - published_claim_len;
    bytes[published_claim_start + 32] ^= 1;
    assert!(matches!(
        WorkerV2LoadEnvelopeV1::from_bytes(&bytes),
        Err(EnvelopeDecodeError::PublishedClaim(
            DurablePublishedClaimCodecErrorV1::ChecksumMismatch
        ))
    ));
}

#[test]
fn published_claim_must_bind_the_direct_link_evidence() {
    let parts = build_parts_with_upstream(Some([0xee; 32]));
    assert!(matches!(
        parts.into_envelope(),
        Err(EnvelopeValidationError::PublicationClaimMismatch(
            fe2o3_worker_v2_bundle::PublicationClaimFieldV1::UpstreamEvidence
        ))
    ));
}

#[test]
fn oversized_counts_and_fields_are_rejected_before_allocation() {
    let mut raw = build_parts().into_envelope().unwrap().to_bytes();
    raw[32..36].copy_from_slice(&((MAX_WORKER_V2_RAW_HSACO_BYTES as u32) + 1).to_le_bytes());
    assert!(matches!(
        WorkerV2LoadEnvelopeV1::from_bytes(&raw),
        Err(EnvelopeDecodeError::LengthOutOfRange {
            field: "raw HSACO",
            ..
        })
    ));

    let mut proofs = build_parts().into_envelope().unwrap().to_bytes();
    proofs[36..38].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        WorkerV2LoadEnvelopeV1::from_bytes(&proofs),
        Err(EnvelopeDecodeError::CountOutOfRange {
            field: "proof records",
            ..
        })
    ));

    let mut proof_length = build_parts().into_envelope().unwrap().to_bytes();
    let (_, proof_start, _) = component_offsets(&proof_length);
    proof_length[proof_start..proof_start + 4]
        .copy_from_slice(&((fe2o3_artifacts::MAX_PROOF_RECORD_BYTES as u32) + 1).to_le_bytes());
    assert!(matches!(
        WorkerV2LoadEnvelopeV1::from_bytes(&proof_length),
        Err(EnvelopeDecodeError::LengthOutOfRange {
            field: "proof record",
            ..
        })
    ));
}

#[test]
fn pre_envelope_inputs_are_canonical_bounded_and_inert() {
    let parts = build_parts();
    let inputs = WorkerV2EnvelopeInputsV1::new(parts.evidence, parts.proofs, parts.raw).unwrap();
    let bytes = inputs.to_bytes();
    let recovered = WorkerV2EnvelopeInputsV1::from_bytes(&bytes).unwrap();
    assert_eq!(recovered, inputs);
    assert_eq!(recovered.identity(), inputs.identity());
    assert!(!recovered.grants_currentness_authority());
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());

    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert!(WorkerV2EnvelopeInputsV1::from_bytes(&changed).is_err());

    let mut trailing = bytes;
    trailing.push(0);
    assert!(matches!(
        WorkerV2EnvelopeInputsV1::from_bytes(&trailing),
        Err(EnvelopeInputsDecodeError::TrailingBytes)
    ));
}
