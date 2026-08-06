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
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
    inspect_worker_v2_raw_hsaco_v1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};

include!("fixtures/worker_v2_hsaco_test_support.rs");

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v2-hsaco-publication-{}-{}",
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

#[derive(Debug, Eq, PartialEq)]
struct PublishedIdentities {
    package: [u8; 32],
    kernel_set: [u8; 32],
    target: [u8; 32],
    request: [u8; 32],
    worker: [u8; 32],
    response: [u8; 32],
    linked_output: [u8; 32],
    finalization: [u8; 32],
    finalized_output: [u8; 32],
    publication: [u8; 32],
    upstream: [u8; 32],
}

#[test]
fn typed_bridge_publishes_exact_inspected_bytes_and_recovers_exact_retry() {
    let directory = TestDirectory::new();
    let producer = publication_producer("tests/typed-bridge-success.rs");
    let fixture = fixture(FixtureOptions::valid());
    let evidence = publication_evidence(
        &directory,
        &producer,
        fixture.bytes.clone(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
    );
    let inspected = inspect_worker_v2_raw_hsaco_v1(evidence).unwrap();
    let inspection_identity = *inspected.identity().as_bytes();
    let attempt = inspected.attempt();
    let prepared =
        fe2o3_hsaco_finalize::prepare_worker_v2_hsaco_publication_v1(&producer, inspected).unwrap();

    assert_eq!(prepared.attempt(), attempt);
    assert_eq!(prepared.exact_bytes(), fixture.bytes);
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());

    let published = fe2o3_hsaco_finalize::publish_prepared_worker_v2_hsaco_v1(
        &directory.0,
        &producer,
        &prepared,
    )
    .unwrap();
    assert_eq!(published.snapshot().artifact().bytes(), fixture.bytes);
    assert_eq!(
        published.snapshot().record().scope().package(),
        fe2o3_artifact_transaction::producer_package_identity_v1(&producer)
    );
    assert_eq!(
        published.receipt().upstream_evidence_identity(),
        inspection_identity
    );
    assert!(!published.snapshot().grants_load_authority());
    assert!(!published.snapshot().grants_launch_authority());

    assert!(matches!(
        fe2o3_hsaco_finalize::publish_prepared_worker_v2_hsaco_v1(
            &directory.0,
            &producer,
            &prepared,
        ),
        Err(fe2o3_hsaco_finalize::WorkerV2HsacoPublicationError::Publication(
            fe2o3_artifact_transaction::AttemptScopedHsacoPublicationErrorV1::ReceiptAlreadyPersisted { .. }
        ))
    ));
    fe2o3_artifact_transaction::finish_build_attempt(&directory.0, &producer, attempt).unwrap();
}

#[test]
fn producer_package_helper_is_domain_separated_and_non_authoritative() {
    let first = publication_producer("tests/package-a.rs");
    let same = publication_producer("tests/package-a.rs");
    let other_source = publication_producer("tests/package-b.rs");
    let other_crate = ProducerIdentity::from_codegen(
        "worker_v2_hsaco_publication_other",
        Some(Path::new("tests/package-a.rs")),
    )
    .unwrap();

    assert_eq!(
        fe2o3_artifact_transaction::producer_package_identity_v1(&first),
        fe2o3_artifact_transaction::producer_package_identity_v1(&same)
    );
    assert_ne!(
        fe2o3_artifact_transaction::producer_package_identity_v1(&first),
        fe2o3_artifact_transaction::producer_package_identity_v1(&other_source)
    );
    assert_ne!(
        fe2o3_artifact_transaction::producer_package_identity_v1(&first),
        fe2o3_artifact_transaction::producer_package_identity_v1(&other_crate)
    );
}

#[test]
fn every_mutated_lineage_changes_its_derived_identity_chain() {
    let base = publish_identities(
        "tests/identity-base.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        false,
    );
    let package = publish_identities(
        "tests/identity-other-source.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        false,
    );
    assert_ne!(base.package, package.package);
    assert_ne!(base.publication, package.publication);

    let manifest = publish_identities(
        "tests/identity-base.rs",
        FixtureOptions {
            entry: "vecsub",
            descriptor: "vecsub.kd",
            ..FixtureOptions::valid()
        },
        "vecsub",
        "vecsub.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        false,
    );
    assert_ne!(base.kernel_set, manifest.kernel_set);
    assert_ne!(base.request, manifest.request);
    assert_ne!(base.publication, manifest.publication);

    let envelope = publish_identities(
        "tests/identity-base.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x54,
        "fixture-llvm-v1",
        false,
    );
    assert_ne!(base.kernel_set, envelope.kernel_set);
    assert_ne!(base.request, envelope.request);
    assert_ne!(base.response, envelope.response);
    assert_ne!(base.publication, envelope.publication);

    let target = publish_identities(
        "tests/identity-base.rs",
        FixtureOptions {
            target: "gfx942:xnack-",
            ..FixtureOptions::valid()
        },
        "vecadd",
        "vecadd.kd",
        "gfx942:xnack-",
        0x53,
        "fixture-llvm-v1",
        false,
    );
    assert_ne!(base.target, target.target);
    assert_ne!(base.request, target.request);
    assert_ne!(base.publication, target.publication);

    let worker = publish_identities(
        "tests/identity-base.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v2",
        false,
    );
    assert_ne!(base.worker, worker.worker);
    assert_ne!(base.request, worker.request);
    assert_ne!(base.response, worker.response);
    assert_ne!(base.publication, worker.publication);

    let output = publish_identities(
        "tests/identity-base.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        true,
    );
    assert_ne!(base.response, output.response);
    assert_ne!(base.linked_output, output.linked_output);
    assert_ne!(base.finalization, output.finalization);
    assert_ne!(base.finalized_output, output.finalized_output);
    assert_ne!(base.upstream, output.upstream);
    assert_ne!(base.publication, output.publication);
}

#[test]
fn prepared_publication_rejects_a_different_producer_before_backend_claim() {
    let directory = TestDirectory::new();
    let producer = publication_producer("tests/producer-bound.rs");
    let other = publication_producer("tests/producer-impostor.rs");
    let fixture = fixture(FixtureOptions::valid());
    let evidence = publication_evidence(
        &directory,
        &producer,
        fixture.bytes,
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
    );
    let prepared = fe2o3_hsaco_finalize::prepare_worker_v2_hsaco_publication_v1(
        &producer,
        inspect_worker_v2_raw_hsaco_v1(evidence).unwrap(),
    )
    .unwrap();

    assert!(matches!(
        fe2o3_hsaco_finalize::publish_prepared_worker_v2_hsaco_v1(&directory.0, &other, &prepared,),
        Err(fe2o3_hsaco_finalize::WorkerV2HsacoPublicationError::ProducerIdentityMismatch)
    ));
}

#[test]
fn request_identity_binds_the_explicit_retained_link_plan_identity() {
    let first_directory = TestDirectory::new();
    let second_directory = TestDirectory::new();
    let producer = publication_producer("tests/link-plan-bound.rs");
    let first_bytes = fixture(FixtureOptions::valid()).bytes;
    let second_bytes = first_bytes.clone();
    let first = publication_evidence_with_link_option(
        &first_directory,
        &producer,
        first_bytes,
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        Some("2"),
    );
    let second = publication_evidence_with_link_option(
        &second_directory,
        &producer,
        second_bytes,
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        Some("3"),
    );
    assert_ne!(first.link_plan_identity(), second.link_plan_identity());

    let first = inspect_worker_v2_raw_hsaco_v1(first).unwrap();
    let second = inspect_worker_v2_raw_hsaco_v1(second).unwrap();
    assert_ne!(first.link_plan_identity(), second.link_plan_identity());
    let first =
        fe2o3_hsaco_finalize::prepare_worker_v2_hsaco_publication_v1(&producer, first).unwrap();
    let second =
        fe2o3_hsaco_finalize::prepare_worker_v2_hsaco_publication_v1(&producer, second).unwrap();
    let first = fe2o3_hsaco_finalize::publish_prepared_worker_v2_hsaco_v1(
        &first_directory.0,
        &producer,
        &first,
    )
    .unwrap();
    let second = fe2o3_hsaco_finalize::publish_prepared_worker_v2_hsaco_v1(
        &second_directory.0,
        &producer,
        &second,
    )
    .unwrap();

    assert_ne!(
        first.snapshot().record().request(),
        second.snapshot().record().request()
    );
}

#[allow(clippy::too_many_arguments)]
fn publish_identities(
    source: &str,
    options: FixtureOptions<'_>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    compiler_target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
    mutate_output: bool,
) -> PublishedIdentities {
    let directory = TestDirectory::new();
    let producer = publication_producer(source);
    let mut fixture = fixture(options);
    if mutate_output {
        fixture.bytes[fixture.text_offset] ^= 1;
    }
    let evidence = publication_evidence(
        &directory,
        &producer,
        fixture.bytes,
        manifest_entry,
        manifest_descriptor,
        compiler_target,
        semantic_seed,
        llvm_identity,
    );
    let inspected = inspect_worker_v2_raw_hsaco_v1(evidence).unwrap();
    let prepared =
        fe2o3_hsaco_finalize::prepare_worker_v2_hsaco_publication_v1(&producer, inspected).unwrap();
    let published = fe2o3_hsaco_finalize::publish_prepared_worker_v2_hsaco_v1(
        &directory.0,
        &producer,
        &prepared,
    )
    .unwrap();
    let record = published.snapshot().record();
    let scope = record.scope();
    let identities = PublishedIdentities {
        package: *scope.package().as_bytes(),
        kernel_set: *scope.kernel_set().as_bytes(),
        target: *scope.target().as_bytes(),
        request: *record.request().as_bytes(),
        worker: *record.worker().unwrap().as_bytes(),
        response: *record.response().unwrap().as_bytes(),
        linked_output: *record.linked_output().unwrap().as_bytes(),
        finalization: *record.finalization().unwrap().as_bytes(),
        finalized_output: *record.finalized_output().unwrap().as_bytes(),
        publication: *record.publication().unwrap().as_bytes(),
        upstream: published.receipt().upstream_evidence_identity(),
    };
    fe2o3_artifact_transaction::finish_build_attempt(&directory.0, &producer, prepared.attempt())
        .unwrap();
    identities
}

fn publication_producer(source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen(
        "worker_v2_hsaco_publication_fixture",
        Some(Path::new(source)),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn publication_evidence(
    directory: &TestDirectory,
    producer: &ProducerIdentity,
    bytes: Vec<u8>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    publication_evidence_with_link_option(
        directory,
        producer,
        bytes,
        manifest_entry,
        manifest_descriptor,
        target,
        semantic_seed,
        llvm_identity,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn publication_evidence_with_link_option(
    directory: &TestDirectory,
    producer: &ProducerIdentity,
    bytes: Vec<u8>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
    link_option: Option<&str>,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    let attempt = begin_build_attempt(
        &directory.0,
        producer,
        BuildInvocation::from_bytes([0xb1; 32]),
        BuildSession::from_bytes([0xb2; 16]),
    )
    .unwrap();
    let handoff = publication_compiler_handoff(
        &bytes,
        manifest_entry,
        manifest_descriptor,
        target,
        semantic_seed,
    );
    publish_compiler_module_handoff_v1(&directory.0, producer, attempt, handoff.canonical_bytes())
        .unwrap();
    let consumed = consume_compiler_module_handoff_v1(&directory.0, producer, attempt).unwrap();
    execute_reproducible_first_build_worker_v2(
        consumed,
        &publication_pinned_worker(llvm_identity),
        Vec::new(),
        publication_link_options(link_option),
        WorkerOutputConstraintsV1::new(64 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(2), 16 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap()
}

fn publication_pinned_worker(llvm_identity: &str) -> PinnedWorkerV1 {
    let path = Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-v2-hsaco-fixture"));
    let executable = fs::read(path).unwrap();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&executable),
        "fixture-worker-v2-hsaco-v1",
        llvm_identity,
    )
    .unwrap();
    PinnedWorkerV1::open(path, measurement).unwrap()
}

fn publication_link_options(opt_level: Option<&str>) -> Vec<LinkOptionV1> {
    [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", opt_level.unwrap_or("2")),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
    .collect()
}

fn publication_compiler_handoff(
    bytes: &[u8],
    manifest_entry: &str,
    manifest_descriptor: &str,
    target: &str,
    semantic_seed: u8,
) -> CompilerModuleHandoffV2 {
    const PAYLOAD_MARKER: &[u8] = b"FE2O3/TEST-HSACO-PAYLOAD/V1\0";
    let target = CompilerDeviceTargetV1::parse(target).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, manifest_entry),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            manifest_descriptor,
        ),
        (CompilerModuleSymbolRoleV1::DeviceFfiExport, "ffi_export"),
    ])
    .unwrap();
    let mut envelope =
        CompilerFfiEnvelopeBuilderV1::new(target, CompilerCodeObjectVersion::V6, 1).unwrap();
    envelope
        .push(publication_compiler_contract(target, semantic_seed))
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

fn publication_compiler_contract(
    target: CompilerDeviceTargetV1,
    semantic_seed: u8,
) -> CompilerFfiContractV1 {
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
            "publication_fixture",
            "publication_fixture::ffi_export",
            [0x35; 16],
            "_RINvNtCs1234_19publication_fixture10ffi_export",
        )
        .unwrap(),
        "ffi_export",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}
