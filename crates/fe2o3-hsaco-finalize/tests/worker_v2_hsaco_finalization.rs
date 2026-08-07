#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1,
    DescriptorSourceEvidenceRequirementV1, FinalizationError, LinkOptionV1, PinnedWorkerV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerV2HsacoFinalizationError, execute_reproducible_first_build_worker_v2,
    finalize_inspected_worker_v2_hsaco_v1, inspect_unfinalized, inspect_worker_v2_raw_hsaco_v1,
    verify_finalized,
};
use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion, CompilerIdentityV1,
    DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1,
    DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1,
    KernelId, LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1,
    SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName, encode_device_descriptor_table_v1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};

include!("fixtures/worker_v2_hsaco_test_support.rs");

#[test]
fn missing_descriptor_source_returns_an_owning_fail_closed_blocker() {
    let fixture = fixture(FixtureOptions::valid());
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x41, 0x51)).unwrap();
    let raw_identity = raw.identity();
    let source_identity = raw.source_evidence_identity();
    let output_identity = raw.linked_output_identity();
    let policy_identity = raw.policy().identity();
    let attempt = raw.attempt();

    let blocker = match finalize_inspected_worker_v2_hsaco_v1(raw) {
        Err(WorkerV2HsacoFinalizationError::MissingAuthenticatedDescriptorSourceEvidence(
            blocker,
        )) => blocker,
        result => panic!("expected missing descriptor-source blocker, found {result:?}"),
    };

    assert_eq!(
        blocker.requirement(),
        DescriptorSourceEvidenceRequirementV1::AuthenticatedCanonicalDescriptorTableV1
    );
    assert_eq!(blocker.raw_inspection_identity(), raw_identity);
    assert_eq!(blocker.source_evidence_identity(), source_identity);
    assert_eq!(blocker.raw_output_identity(), output_identity);
    assert_eq!(blocker.policy_identity(), policy_identity);
    assert_eq!(blocker.attempt(), attempt);
    assert_eq!(blocker.target().to_string(), "gfx942");
    assert_eq!(blocker.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(blocker.observed_kernels().len(), 1);
    assert_eq!(blocker.observed_kernels()[0].entry(), "vecadd");
    assert_eq!(
        blocker.canonical_descriptor_section(),
        CanonicalDescriptorSectionObservationV1::Missing
    );
    assert!(!blocker.may_infer_descriptor_claims_from_executable_metadata());
    assert!(!blocker.grants_publication_authority());
    assert!(!blocker.grants_load_authority());
    assert!(!blocker.grants_launch_authority());
}

#[test]
fn structurally_finalizes_and_retains_raw_and_finalized_lineage() {
    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let unfinalized = inspect_unfinalized(&raw_bytes).unwrap();
    let digest_offset = unfinalized.location().digest_offset();
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(raw_bytes.clone(), "gfx942", 0x42, 0x52)).unwrap();
    let raw_identity = raw.identity();
    let source_identity = raw.source_evidence_identity();
    let raw_output = raw.linked_output_identity();
    let policy = raw.policy().identity();

    let prepared = finalize_inspected_worker_v2_hsaco_v1(raw).unwrap();
    let finalized = prepared.exact_finalized_bytes();
    let verified = verify_finalized(finalized).unwrap();

    assert_eq!(prepared.raw_inspection_identity(), raw_identity);
    assert_eq!(prepared.source_evidence_identity(), source_identity);
    assert_eq!(prepared.raw_output_identity(), raw_output);
    assert_eq!(prepared.policy_identity(), policy);
    assert!(raw_output.matches(&raw_bytes));
    assert!(prepared.finalized_output_identity().matches(finalized));
    assert_ne!(
        prepared.raw_output_identity(),
        prepared.finalized_output_identity()
    );
    assert_eq!(prepared.canonical_digest(), verified.digest());
    assert_eq!(prepared.target().to_string(), "gfx942");
    assert_eq!(prepared.code_object_version(), CodeObjectVersion::V6);
    assert!(prepared.canonical_descriptor_finalization_ran());
    assert!(!prepared.has_authenticated_descriptor_source_evidence());
    assert!(prepared.is_structural_only());
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.proves_verus_verification());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());
    assert_ne!(prepared.identity().as_bytes(), &[0; 32]);

    for (index, (before, after)) in raw_bytes.iter().zip(finalized).enumerate() {
        if !(digest_offset..digest_offset + 32).contains(&index) {
            assert_eq!(before, after, "byte {index} outside digest slot changed");
        }
    }
}

#[test]
fn descriptor_and_finalized_byte_tampering_fail_closed() {
    let mut table = descriptor_table("gfx942");
    table[16] = 1;
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x43, 0x53)).unwrap();
    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::ExpectedZeroDigest
        ))
    ));

    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let text_offset = fixture.text_offset;
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x44, 0x54)).unwrap();
    let prepared = finalize_inspected_worker_v2_hsaco_v1(raw).unwrap();
    let mut tampered = prepared.exact_finalized_bytes().to_vec();
    tampered[text_offset] ^= 1;
    assert!(matches!(
        verify_finalized(&tampered),
        Err(FinalizationError::CanonicalDigestMismatch { .. })
    ));
}

#[test]
fn rejects_double_finalization() {
    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x45, 0x55)).unwrap();
    let finalized = finalize_inspected_worker_v2_hsaco_v1(raw)
        .unwrap()
        .exact_finalized_bytes()
        .to_vec();

    let raw = inspect_worker_v2_raw_hsaco_v1(evidence(finalized, "gfx942", 0x46, 0x56)).unwrap();
    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::ExpectedZeroDigest
        ))
    ));
}

#[test]
fn rejects_descriptor_target_mismatch_without_weakening_raw_target_policy() {
    let table = descriptor_table("gfx942:xnack-");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x47, 0x57)).unwrap();

    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::DeviceTargetMismatch
        ))
    ));
}

#[test]
fn finalization_identity_binds_lineage_separately_from_finalized_content() {
    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let first = prepare(fixture.bytes.clone(), "gfx942", 0x48, 0x58);
    let other_lineage = prepare(fixture.bytes.clone(), "gfx942", 0x49, 0x59);

    assert_eq!(
        first.finalized_output_identity(),
        other_lineage.finalized_output_identity()
    );
    assert_eq!(first.canonical_digest(), other_lineage.canonical_digest());
    assert_ne!(
        first.raw_inspection_identity(),
        other_lineage.raw_inspection_identity()
    );
    assert_ne!(first.identity(), other_lineage.identity());

    let mut changed = fixture.bytes;
    changed[fixture.text_offset] ^= 1;
    let changed = prepare(changed, "gfx942", 0x4a, 0x5a);
    assert_ne!(first.raw_output_identity(), changed.raw_output_identity());
    assert_ne!(
        first.finalized_output_identity(),
        changed.finalized_output_identity()
    );
    assert_ne!(first.canonical_digest(), changed.canonical_digest());
    assert_ne!(first.identity(), changed.identity());
}

fn prepare(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
) -> fe2o3_hsaco_finalize::PreparedFinalizedWorkerV2HsacoV1 {
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(bytes, target, invocation_seed, semantic_seed))
            .unwrap();
    finalize_inspected_worker_v2_hsaco_v1(raw).unwrap()
}

fn descriptor_table(target: &str) -> Vec<u8> {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x61; 32]),
        name("vecadd"),
        name("vecadd"),
        name("vecadd.kd"),
        build_evidence(0x62, 0x63),
        build_evidence(0x64, 0x65),
        Vec::new(),
        KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            256,
            0,
            64 * 1024,
        )
        .unwrap(),
        vec![LogicalArgumentV1::shared_slice(0, name("values"), &source, &layout, 0).unwrap()],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc"), text("unauthenticated-test"), [0x66; 20]),
        ProducerIdentityV1::new(text("fe2o3-test"), text("unauthenticated-test")),
        DeviceTargetV1::parse(target).unwrap(),
        vec![source],
        vec![layout],
        vec![kernel],
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

fn name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn build_evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v2-hsaco-finalization-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn evidence(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    let directory = TestDirectory::new();
    let producer = ProducerIdentity::from_codegen(
        "worker_v2_hsaco_finalization_fixture",
        Some(Path::new("tests/worker_v2_hsaco_finalization.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([invocation_seed; 32]),
        BuildSession::from_bytes([invocation_seed.wrapping_add(1); 16]),
    )
    .unwrap();
    let handoff = compiler_handoff(&bytes, target, semantic_seed);
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .unwrap();
    let consumed = consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
    execute_reproducible_first_build_worker_v2(
        consumed,
        &pinned_worker(),
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(64 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(2), 16 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap()
}

fn pinned_worker() -> PinnedWorkerV1 {
    let path = Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-v2-hsaco-fixture"));
    let executable = fs::read(path).unwrap();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&executable),
        "fixture-worker-v2-hsaco-v1",
        "fixture-llvm-v1",
    )
    .unwrap();
    PinnedWorkerV1::open(path, measurement).unwrap()
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", "2"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
    .collect()
}

fn compiler_handoff(bytes: &[u8], target: &str, semantic_seed: u8) -> CompilerModuleHandoffV2 {
    const PAYLOAD_MARKER: &[u8] = b"FE2O3/TEST-HSACO-PAYLOAD/V1\0";
    let target = CompilerDeviceTargetV1::parse(target).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "vecadd"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "vecadd.kd"),
        (CompilerModuleSymbolRoleV1::DeviceFfiExport, "ffi_export"),
    ])
    .unwrap();
    let mut envelope =
        CompilerFfiEnvelopeBuilderV1::new(target, CompilerCodeObjectVersion::V6, 1).unwrap();
    envelope
        .push(compiler_contract(target, semantic_seed))
        .unwrap();
    let mut module = PAYLOAD_MARKER.to_vec();
    module.extend_from_slice(bytes);
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmBitcode,
        target,
        CompilerCodeObjectVersion::V6,
        envelope.finish().unwrap(),
        manifest,
        &module,
    )
    .unwrap()
}

fn compiler_contract(target: CompilerDeviceTargetV1, semantic_seed: u8) -> CompilerFfiContractV1 {
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    let semantic_identity = [semantic_seed; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let target_text = target.to_string();
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "ffi_export",
        calling_convention: "C",
        code_object_version: 6,
        target: &target_text,
        physical_abi: ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    };
    CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Export,
        CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        target,
        CompilerCodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            "finalization_fixture",
            "finalization_fixture::ffi_export",
            [0x67; 16],
            "_RINvNtCs1234_20finalization_fixture10ffi_export",
        )
        .unwrap(),
        "ffi_export",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}
