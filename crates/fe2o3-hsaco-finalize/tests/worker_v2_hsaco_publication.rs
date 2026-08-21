#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, CompilerModuleHandoffSlotV2, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_in_slot_v2,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_in_slot_v2,
    publish_compiler_module_handoff_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, ProtectedWorkerV2HsacoPublicationRouteV2,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerV2HsacoPublicationRouteV1, execute_protected_reproducible_first_build_worker_v2,
    execute_reproducible_first_build_worker_v2, finalize_inspected_protected_worker_v2_hsaco_v2,
    finalize_inspected_worker_v2_hsaco_v1, inspect_protected_worker_v2_raw_hsaco_v1,
    inspect_worker_v2_raw_hsaco_v1,
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
fn protected_raw_preparation_exposes_exact_v2_restart_inputs() {
    let directory = TestDirectory::new();
    let producer = publication_producer("tests/protected-raw-preparation.rs");
    let fixture = fixture(FixtureOptions::valid());
    let closure = publication_compiler_closure(0x21);
    let slot = CompilerModuleHandoffSlotV2::GeneralGemmReference;
    let inspected = inspect_protected_worker_v2_raw_hsaco_v1(protected_publication_evidence(
        &directory,
        &producer,
        fixture.bytes.clone(),
        closure,
        slot,
        0xb1,
        0xb2,
    ))
    .unwrap();
    let attempt = inspected.attempt();
    let handoff = inspected.handoff_identity();
    let inspection = inspected.identity();
    let prepared = fe2o3_hsaco_finalize::prepare_protected_worker_v2_hsaco_publication_v2(
        &producer, inspected,
    )
    .unwrap();
    let intent = prepared.publication_intent();

    assert_eq!(prepared.attempt(), attempt);
    assert_eq!(prepared.handoff_slot(), slot);
    assert_eq!(prepared.handoff_identity(), handoff);
    assert_eq!(prepared.compiler_closure(), closure);
    assert_eq!(prepared.exact_retained_output(), fixture.bytes);
    assert_eq!(
        intent.route(),
        ProtectedWorkerV2HsacoPublicationRouteV2::InspectedRaw
    );
    assert_eq!(intent.raw_inspection_identity(), inspection);
    assert_eq!(intent.canonical_finalization_identity(), None);
    assert_eq!(intent.handoff_slot(), slot);
    assert_eq!(intent.handoff_identity(), handoff);
    assert_eq!(intent.compiler_closure(), closure);
    assert_eq!(intent.durable_plan().attempt(), attempt);
    assert_eq!(
        intent.durable_plan().linked_output().as_bytes(),
        intent.raw_linked_snapshot_identity().sha256()
    );
    assert_eq!(
        intent.durable_plan().finalized_output().as_bytes(),
        intent.retained_snapshot_identity().sha256()
    );
    assert!(intent.matches_exact_retained_output(prepared.exact_retained_output()));
    let mut mutated = prepared.exact_retained_output().to_vec();
    mutated[0] ^= 1;
    assert!(!intent.matches_exact_retained_output(&mutated));
    assert!(!prepared.grants_compiler_authority());
    assert!(!prepared.grants_publication_authority());
    assert!(!intent.grants_compiler_authority());
    assert!(!intent.grants_publication_authority());
    assert!(!intent.grants_load_authority());
    assert!(!intent.grants_launch_authority());
}

#[test]
fn protected_finalized_preparation_retains_raw_and_exact_canonical_snapshots() {
    let directory = TestDirectory::new();
    let producer = publication_producer("tests/protected-finalized-preparation.rs");
    let table = publication_descriptor_table("gfx942", "vecadd", "vecadd.kd");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let closure = publication_compiler_closure(0x31);
    let slot = CompilerModuleHandoffSlotV2::GeneralGemmVectorizedAOnly;
    let inspected = inspect_protected_worker_v2_raw_hsaco_v1(protected_publication_evidence(
        &directory,
        &producer,
        fixture.bytes,
        closure,
        slot,
        0xc1,
        0xc2,
    ))
    .unwrap();
    let inspection = inspected.identity();
    let finalized = finalize_inspected_protected_worker_v2_hsaco_v2(inspected).unwrap();
    let finalization = finalized.identity();
    let finalized_bytes = finalized.exact_finalized_bytes().to_vec();
    let prepared =
        fe2o3_hsaco_finalize::prepare_finalized_protected_worker_v2_hsaco_publication_v2(
            &producer, finalized,
        )
        .unwrap();
    let intent = prepared.publication_intent();

    assert_eq!(prepared.handoff_slot(), slot);
    assert_eq!(prepared.compiler_closure(), closure);
    assert_eq!(prepared.exact_retained_output(), finalized_bytes);
    assert_eq!(
        intent.route(),
        ProtectedWorkerV2HsacoPublicationRouteV2::CanonicallyFinalized
    );
    assert_eq!(intent.raw_inspection_identity(), inspection);
    assert_eq!(intent.canonical_finalization_identity(), Some(finalization));
    assert!(intent.raw_linked_snapshot_identity().matches(&raw_bytes));
    assert!(
        intent
            .retained_snapshot_identity()
            .matches(prepared.exact_retained_output())
    );
    assert_ne!(
        intent.raw_linked_snapshot_identity(),
        intent.retained_snapshot_identity()
    );
    assert_eq!(
        intent.durable_plan().linked_output().as_bytes(),
        intent.raw_linked_snapshot_identity().sha256()
    );
    assert_eq!(
        intent.durable_plan().finalized_output().as_bytes(),
        intent.retained_snapshot_identity().sha256()
    );
}

#[test]
fn protected_domains_reject_v1_aliasing_and_bind_every_closure_role() {
    let base = protected_raw_plan_identities(publication_compiler_closure(0x41));
    for role in 0..6 {
        let changed = protected_raw_plan_identities(publication_mutated_closure(0x41, role));
        assert_eq!(base.linked_output, changed.linked_output);
        assert_eq!(base.finalized_output, changed.finalized_output);
        assert_ne!(base.request, changed.request);
        assert_ne!(base.finalization, changed.finalization);
        assert_ne!(base.publication, changed.publication);
        assert_ne!(base.upstream, changed.upstream);
    }

    let protected_directory = TestDirectory::new();
    let ordinary_directory = TestDirectory::new();
    let producer = publication_producer("tests/protected-v1-domain-separation.rs");
    let fixture = fixture(FixtureOptions::valid());
    let protected = inspect_protected_worker_v2_raw_hsaco_v1(protected_publication_evidence(
        &protected_directory,
        &producer,
        fixture.bytes.clone(),
        publication_compiler_closure(0x41),
        CompilerModuleHandoffSlotV2::Default,
        0xb1,
        0xb2,
    ))
    .unwrap();
    let protected = fe2o3_hsaco_finalize::prepare_protected_worker_v2_hsaco_publication_v2(
        &producer, protected,
    )
    .unwrap()
    .publication_intent();
    let ordinary = inspect_worker_v2_raw_hsaco_v1(publication_evidence(
        &ordinary_directory,
        &producer,
        fixture.bytes,
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
    ))
    .unwrap();
    let ordinary =
        fe2o3_hsaco_finalize::prepare_worker_v2_hsaco_publication_v1(&producer, ordinary)
            .unwrap()
            .publication_intent();
    let protected_plan = protected.durable_plan();
    let ordinary_plan = ordinary.durable_plan();
    assert_eq!(
        protected_plan.linked_output(),
        ordinary_plan.linked_output()
    );
    assert_eq!(
        protected_plan.finalized_output(),
        ordinary_plan.finalized_output()
    );
    assert_ne!(
        protected_plan.scope().kernel_set(),
        ordinary_plan.scope().kernel_set()
    );
    assert_ne!(
        protected_plan.scope().target(),
        ordinary_plan.scope().target()
    );
    assert_ne!(protected_plan.request(), ordinary_plan.request());
    assert_ne!(protected_plan.worker(), ordinary_plan.worker());
    assert_ne!(protected_plan.response(), ordinary_plan.response());
    assert_ne!(protected_plan.finalization(), ordinary_plan.finalization());
    assert_ne!(protected_plan.publication(), ordinary_plan.publication());
    assert_ne!(protected.upstream_evidence(), ordinary.upstream_evidence());
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
    let intent = prepared.publication_intent();
    assert_eq!(
        intent.route(),
        WorkerV2HsacoPublicationRouteV1::InspectedRaw
    );
    assert_eq!(
        intent.raw_inspection_identity().as_bytes(),
        &inspection_identity
    );
    assert_eq!(intent.canonical_finalization_identity(), None);
    assert_eq!(
        intent.raw_linked_snapshot_identity(),
        intent.finalized_snapshot_identity()
    );
    assert_eq!(
        intent.durable_plan().linked_output().as_bytes(),
        intent.raw_linked_snapshot_identity().sha256()
    );
    assert_eq!(
        intent.durable_plan().finalized_output().as_bytes(),
        intent.finalized_snapshot_identity().sha256()
    );
    assert!(intent.matches_exact_retained_output(prepared.exact_bytes()));
    let mut substituted_raw = prepared.exact_bytes().to_vec();
    substituted_raw[0] ^= 1;
    assert!(!intent.matches_exact_retained_output(&substituted_raw));
    assert!(!intent.authenticates_compiler_origin());
    assert!(!intent.grants_publication_authority());
    assert!(!intent.grants_load_authority());
    assert!(!intent.grants_launch_authority());
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
    assert_eq!(
        published.receipt().upstream_evidence_identity(),
        intent.upstream_evidence().as_bytes()
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
fn finalized_bridge_publishes_only_canonical_bytes_and_retries_exact_plan() {
    let directory = TestDirectory::new();
    let producer = publication_producer("tests/finalized-typed-bridge.rs");
    let table = publication_descriptor_table("gfx942", "vecadd", "vecadd.kd");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let evidence = publication_evidence(
        &directory,
        &producer,
        raw_bytes.clone(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
    );
    let finalized =
        finalize_inspected_worker_v2_hsaco_v1(inspect_worker_v2_raw_hsaco_v1(evidence).unwrap())
            .unwrap();
    let raw_identity = finalized.raw_inspection_identity();
    let finalization_identity = finalized.identity();
    let prepared = fe2o3_hsaco_finalize::prepare_finalized_worker_v2_hsaco_publication_v1(
        &producer, finalized,
    )
    .unwrap();

    assert_eq!(prepared.raw_inspection_identity(), raw_identity);
    assert_eq!(
        prepared.canonical_finalization_identity(),
        finalization_identity
    );
    assert!(prepared.raw_output_identity().matches(&raw_bytes));
    assert!(
        prepared
            .finalized_output_identity()
            .matches(prepared.exact_finalized_bytes())
    );
    assert_ne!(prepared.exact_finalized_bytes(), raw_bytes);
    let intent = prepared.publication_intent();
    assert_eq!(
        intent.route(),
        WorkerV2HsacoPublicationRouteV1::CanonicallyFinalized
    );
    assert_eq!(intent.raw_inspection_identity(), raw_identity);
    assert_eq!(
        intent.canonical_finalization_identity(),
        Some(finalization_identity)
    );
    assert_eq!(
        intent.raw_linked_snapshot_identity(),
        prepared.raw_output_identity()
    );
    assert_eq!(
        intent.finalized_snapshot_identity(),
        prepared.finalized_output_identity()
    );
    assert_ne!(
        intent.raw_linked_snapshot_identity(),
        intent.finalized_snapshot_identity()
    );
    assert_eq!(
        intent.durable_plan().linked_output().as_bytes(),
        intent.raw_linked_snapshot_identity().sha256()
    );
    assert_eq!(
        intent.durable_plan().finalized_output().as_bytes(),
        intent.finalized_snapshot_identity().sha256()
    );
    assert!(intent.matches_exact_retained_output(prepared.exact_finalized_bytes()));
    assert!(!intent.matches_exact_retained_output(&raw_bytes));
    let mut substituted_finalized = prepared.exact_finalized_bytes().to_vec();
    substituted_finalized[0] ^= 1;
    assert!(!intent.matches_exact_retained_output(&substituted_finalized));
    assert!(!intent.authenticates_compiler_origin());
    assert!(!intent.grants_publication_authority());
    assert!(!intent.grants_load_authority());
    assert!(!intent.grants_launch_authority());
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.proves_verus_verification());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());
    let other_producer = publication_producer("tests/finalized-producer-swap.rs");
    assert!(matches!(
        fe2o3_hsaco_finalize::publish_prepared_finalized_worker_v2_hsaco_v1(
            &directory.0,
            &other_producer,
            &prepared,
        ),
        Err(fe2o3_hsaco_finalize::WorkerV2HsacoPublicationError::ProducerIdentityMismatch)
    ));

    let published = fe2o3_hsaco_finalize::publish_prepared_finalized_worker_v2_hsaco_v1(
        &directory.0,
        &producer,
        &prepared,
    )
    .unwrap();
    assert_eq!(
        published.snapshot().artifact().bytes(),
        prepared.exact_finalized_bytes()
    );
    assert_ne!(published.snapshot().artifact().bytes(), raw_bytes);
    assert_ne!(
        published.receipt().upstream_evidence_identity(),
        *raw_identity.as_bytes()
    );
    assert_eq!(
        published.receipt().upstream_evidence_identity(),
        intent.upstream_evidence().as_bytes()
    );
    assert!(matches!(
        fe2o3_hsaco_finalize::publish_prepared_finalized_worker_v2_hsaco_v1(
            &directory.0,
            &producer,
            &prepared,
        ),
        Err(fe2o3_hsaco_finalize::WorkerV2HsacoPublicationError::Publication(
            fe2o3_artifact_transaction::AttemptScopedHsacoPublicationErrorV1::ReceiptAlreadyPersisted { .. }
        ))
    ));
    fe2o3_artifact_transaction::finish_build_attempt(&directory.0, &producer, prepared.attempt())
        .unwrap();
}

#[test]
fn raw_and_finalized_publication_have_domain_separated_receipts() {
    let raw_directory = TestDirectory::new();
    let finalized_directory = TestDirectory::new();
    let producer = publication_producer("tests/raw-final-domain-separation.rs");
    let table = publication_descriptor_table("gfx942", "vecadd", "vecadd.kd");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));

    let raw_evidence = publication_evidence(
        &raw_directory,
        &producer,
        fixture.bytes.clone(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
    );
    let raw_prepared = fe2o3_hsaco_finalize::prepare_worker_v2_hsaco_publication_v1(
        &producer,
        inspect_worker_v2_raw_hsaco_v1(raw_evidence).unwrap(),
    )
    .unwrap();
    let raw_published = fe2o3_hsaco_finalize::publish_prepared_worker_v2_hsaco_v1(
        &raw_directory.0,
        &producer,
        &raw_prepared,
    )
    .unwrap();

    let finalized_evidence = publication_evidence(
        &finalized_directory,
        &producer,
        fixture.bytes.clone(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
    );
    let finalized = finalize_inspected_worker_v2_hsaco_v1(
        inspect_worker_v2_raw_hsaco_v1(finalized_evidence).unwrap(),
    )
    .unwrap();
    let finalized_prepared =
        fe2o3_hsaco_finalize::prepare_finalized_worker_v2_hsaco_publication_v1(
            &producer, finalized,
        )
        .unwrap();
    let finalized_published = fe2o3_hsaco_finalize::publish_prepared_finalized_worker_v2_hsaco_v1(
        &finalized_directory.0,
        &producer,
        &finalized_prepared,
    )
    .unwrap();

    let raw_record = raw_published.snapshot().record();
    let finalized_record = finalized_published.snapshot().record();
    assert_eq!(raw_record.linked_output(), finalized_record.linked_output());
    assert_ne!(
        raw_record.scope().kernel_set(),
        finalized_record.scope().kernel_set()
    );
    assert_ne!(
        raw_record.scope().target(),
        finalized_record.scope().target()
    );
    assert_ne!(raw_record.request(), finalized_record.request());
    assert_ne!(raw_record.worker(), finalized_record.worker());
    assert_ne!(raw_record.response(), finalized_record.response());
    assert_ne!(raw_record.finalization(), finalized_record.finalization());
    assert_ne!(
        raw_record.finalized_output(),
        finalized_record.finalized_output()
    );
    assert_ne!(raw_record.publication(), finalized_record.publication());
    assert_ne!(raw_published.receipt(), finalized_published.receipt());
    assert_ne!(
        raw_published.receipt().plan_commitment(),
        finalized_published.receipt().plan_commitment()
    );

    fe2o3_artifact_transaction::finish_build_attempt(
        &raw_directory.0,
        &producer,
        raw_prepared.attempt(),
    )
    .unwrap();
    fe2o3_artifact_transaction::finish_build_attempt(
        &finalized_directory.0,
        &producer,
        finalized_prepared.attempt(),
    )
    .unwrap();
}

#[test]
fn finalized_plan_binds_kernel_target_worker_attempt_and_producer() {
    let base = publish_finalized_identities(
        "tests/finalized-lineage.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        0xb1,
        0xb2,
    );
    let kernel = publish_finalized_identities(
        "tests/finalized-lineage.rs",
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
        0xb1,
        0xb2,
    );
    assert_ne!(base.kernel_set, kernel.kernel_set);
    assert_ne!(base.publication, kernel.publication);

    let target = publish_finalized_identities(
        "tests/finalized-lineage.rs",
        FixtureOptions {
            target: "gfx942:xnack-",
            ..FixtureOptions::valid()
        },
        "vecadd",
        "vecadd.kd",
        "gfx942:xnack-",
        0x53,
        "fixture-llvm-v1",
        0xb1,
        0xb2,
    );
    assert_ne!(base.target, target.target);
    assert_ne!(base.publication, target.publication);

    let worker = publish_finalized_identities(
        "tests/finalized-lineage.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v2",
        0xb1,
        0xb2,
    );
    assert_ne!(base.worker, worker.worker);
    assert_ne!(base.publication, worker.publication);

    let attempt = publish_finalized_identities(
        "tests/finalized-lineage.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        0xc1,
        0xc2,
    );
    assert_ne!(base.publication, attempt.publication);

    let producer = publish_finalized_identities(
        "tests/finalized-other-producer.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        0xb1,
        0xb2,
    );
    assert_ne!(base.package, producer.package);
    assert_ne!(base.publication, producer.publication);

    let changed_bytes = publish_finalized_identities_with_output_mutation(
        "tests/finalized-lineage.rs",
        FixtureOptions::valid(),
        "vecadd",
        "vecadd.kd",
        "gfx942",
        0x53,
        "fixture-llvm-v1",
        0xb1,
        0xb2,
    );
    assert_ne!(base.linked_output, changed_bytes.linked_output);
    assert_ne!(base.finalization, changed_bytes.finalization);
    assert_ne!(base.finalized_output, changed_bytes.finalized_output);
    assert_ne!(base.upstream, changed_bytes.upstream);
    assert_ne!(base.publication, changed_bytes.publication);
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

#[allow(clippy::too_many_arguments)]
fn publish_finalized_identities(
    source: &str,
    options: FixtureOptions<'_>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    compiler_target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
    invocation_seed: u8,
    session_seed: u8,
) -> PublishedIdentities {
    publish_finalized_identities_inner(
        source,
        options,
        manifest_entry,
        manifest_descriptor,
        compiler_target,
        semantic_seed,
        llvm_identity,
        invocation_seed,
        session_seed,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn publish_finalized_identities_with_output_mutation(
    source: &str,
    options: FixtureOptions<'_>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    compiler_target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
    invocation_seed: u8,
    session_seed: u8,
) -> PublishedIdentities {
    publish_finalized_identities_inner(
        source,
        options,
        manifest_entry,
        manifest_descriptor,
        compiler_target,
        semantic_seed,
        llvm_identity,
        invocation_seed,
        session_seed,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn publish_finalized_identities_inner(
    source: &str,
    options: FixtureOptions<'_>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    compiler_target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
    invocation_seed: u8,
    session_seed: u8,
    mutate_output: bool,
) -> PublishedIdentities {
    let directory = TestDirectory::new();
    let producer = publication_producer(source);
    let table = publication_descriptor_table(compiler_target, manifest_entry, manifest_descriptor);
    let mut fixture = fixture_with_descriptor_table(options, Some(&table));
    if mutate_output {
        fixture.bytes[fixture.text_offset] ^= 1;
    }
    let evidence = publication_evidence_with_attempt(
        &directory,
        &producer,
        fixture.bytes,
        manifest_entry,
        manifest_descriptor,
        compiler_target,
        semantic_seed,
        llvm_identity,
        invocation_seed,
        session_seed,
    );
    let finalized =
        finalize_inspected_worker_v2_hsaco_v1(inspect_worker_v2_raw_hsaco_v1(evidence).unwrap())
            .unwrap();
    let prepared = fe2o3_hsaco_finalize::prepare_finalized_worker_v2_hsaco_publication_v1(
        &producer, finalized,
    )
    .unwrap();
    let published = fe2o3_hsaco_finalize::publish_prepared_finalized_worker_v2_hsaco_v1(
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

fn protected_raw_plan_identities(closure: CompilerClosureV2) -> PublishedIdentities {
    let directory = TestDirectory::new();
    let producer = publication_producer("tests/protected-closure-mutations.rs");
    let fixture = fixture(FixtureOptions::valid());
    let inspected = inspect_protected_worker_v2_raw_hsaco_v1(protected_publication_evidence(
        &directory,
        &producer,
        fixture.bytes,
        closure,
        CompilerModuleHandoffSlotV2::Default,
        0xd1,
        0xd2,
    ))
    .unwrap();
    let intent = fe2o3_hsaco_finalize::prepare_protected_worker_v2_hsaco_publication_v2(
        &producer, inspected,
    )
    .unwrap()
    .publication_intent();
    let plan = intent.durable_plan();
    PublishedIdentities {
        package: *plan.scope().package().as_bytes(),
        kernel_set: *plan.scope().kernel_set().as_bytes(),
        target: *plan.scope().target().as_bytes(),
        request: *plan.request().as_bytes(),
        worker: *plan.worker().as_bytes(),
        response: *plan.response().as_bytes(),
        linked_output: *plan.linked_output().as_bytes(),
        finalization: *plan.finalization().as_bytes(),
        finalized_output: *plan.finalized_output().as_bytes(),
        publication: *plan.publication().as_bytes(),
        upstream: intent.upstream_evidence().as_bytes(),
    }
}

fn publication_producer(source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen(
        "worker_v2_hsaco_publication_fixture",
        Some(Path::new(source)),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn protected_publication_evidence(
    directory: &TestDirectory,
    producer: &ProducerIdentity,
    bytes: Vec<u8>,
    closure: CompilerClosureV2,
    slot: CompilerModuleHandoffSlotV2,
    invocation_seed: u8,
    session_seed: u8,
) -> fe2o3_hsaco_finalize::InertProtectedFirstBuildWorkerV2EvidenceV1 {
    let attempt = begin_build_attempt(
        &directory.0,
        producer,
        BuildInvocation::from_bytes([invocation_seed; 32]),
        BuildSession::from_bytes([session_seed; 16]),
    )
    .unwrap();
    let handoff = publication_compiler_handoff(&bytes, "vecadd", "vecadd.kd", "gfx942", 0x53);
    publish_compiler_module_handoff_in_slot_v2(
        &directory.0,
        producer,
        attempt,
        slot,
        closure,
        handoff.canonical_bytes(),
    )
    .unwrap();
    let consumed =
        consume_compiler_module_handoff_in_slot_v2(&directory.0, producer, attempt, slot, closure)
            .unwrap();
    execute_protected_reproducible_first_build_worker_v2(
        consumed,
        &publication_pinned_worker("fixture-llvm-v1"),
        Vec::new(),
        publication_link_options(None),
        WorkerOutputConstraintsV1::new(64 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(2), 16 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap()
}

fn publication_compiler_closure(seed: u8) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
    )
    .unwrap()
}

fn publication_mutated_closure(seed: u8, role: usize) -> CompilerClosureV2 {
    let mut pins = [
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
    ];
    pins[role][0] ^= 0xff;
    CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
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
    publication_evidence_with_link_option_and_attempt(
        directory,
        producer,
        bytes,
        manifest_entry,
        manifest_descriptor,
        target,
        semantic_seed,
        llvm_identity,
        link_option,
        0xb1,
        0xb2,
    )
}

#[allow(clippy::too_many_arguments)]
fn publication_evidence_with_attempt(
    directory: &TestDirectory,
    producer: &ProducerIdentity,
    bytes: Vec<u8>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
    invocation_seed: u8,
    session_seed: u8,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    publication_evidence_with_link_option_and_attempt(
        directory,
        producer,
        bytes,
        manifest_entry,
        manifest_descriptor,
        target,
        semantic_seed,
        llvm_identity,
        None,
        invocation_seed,
        session_seed,
    )
}

#[allow(clippy::too_many_arguments)]
fn publication_evidence_with_link_option_and_attempt(
    directory: &TestDirectory,
    producer: &ProducerIdentity,
    bytes: Vec<u8>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    target: &str,
    semantic_seed: u8,
    llvm_identity: &str,
    link_option: Option<&str>,
    invocation_seed: u8,
    session_seed: u8,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    let attempt = begin_build_attempt(
        &directory.0,
        producer,
        BuildInvocation::from_bytes([invocation_seed; 32]),
        BuildSession::from_bytes([session_seed; 16]),
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

fn publication_descriptor_table(target: &str, entry: &str, descriptor: &str) -> Vec<u8> {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x61; 32]),
        publication_name(entry),
        publication_name(entry),
        publication_name(descriptor),
        publication_build_evidence(0x62, 0x63),
        publication_build_evidence(0x64, 0x65),
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
        vec![
            LogicalArgumentV1::shared_slice(0, publication_name("values"), &source, &layout, 0)
                .unwrap(),
        ],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            publication_text("rustc"),
            publication_text("unauthenticated-test"),
            [0x66; 20],
        ),
        ProducerIdentityV1::new(
            publication_text("fe2o3-test"),
            publication_text("unauthenticated-test"),
        ),
        DeviceTargetV1::parse(target).unwrap(),
        vec![source],
        vec![layout],
        vec![kernel],
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

fn publication_name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn publication_text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn publication_build_evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
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
