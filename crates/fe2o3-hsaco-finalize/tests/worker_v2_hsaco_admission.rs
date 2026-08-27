#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, CompilerModuleHandoffIdentityV2, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, consume_compiler_module_handoff_v2,
    publish_compiler_module_handoff_v1, publish_compiler_module_handoff_v2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1, DEVICE_DESCRIPTOR_SECTION_NAME,
    LinkOptionV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, WorkerV2RawHsacoInspectionError,
    execute_protected_reproducible_first_build_worker_v2,
    execute_reproducible_first_build_worker_v2,
    inspect_protected_production_v1_worker_v2_raw_hsaco_v1,
    inspect_protected_worker_v2_raw_hsaco_v1, inspect_worker_v2_raw_hsaco_v1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};
use rmpv::{Value, encode::write_value};

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const SECTION_HEADER_BYTES: usize = 64;
const NOTE_SECTION_INDEX: usize = 1;
const RODATA_SECTION_INDEX: usize = 2;
const TEXT_SECTION_INDEX: usize = 3;
const STRTAB_SECTION_INDEX: usize = 4;
const SYMTAB_SECTION_INDEX: usize = 5;
const SHSTRTAB_SECTION_INDEX: usize = 6;
const TARGET: &str = "gfx942:xnack-";

#[derive(Clone, Copy)]
struct FixtureOptions<'a> {
    target: &'a str,
    code_object_version: u8,
    entry: &'a str,
    descriptor: &'a str,
    required_workgroup_size: [u32; 3],
    max_flat_workgroup_size: u32,
    wavefront_size: u32,
    descriptor_wavefront_size: u32,
    kernarg_segment_alignment: u64,
    group_segment_fixed_size: u32,
    include_export: bool,
    include_canonical_descriptor_section_name: bool,
}

impl FixtureOptions<'static> {
    fn valid() -> Self {
        Self {
            target: TARGET,
            code_object_version: 4,
            entry: "vecadd",
            descriptor: "vecadd.kd",
            required_workgroup_size: [256, 1, 1],
            max_flat_workgroup_size: 256,
            wavefront_size: 64,
            descriptor_wavefront_size: 64,
            kernarg_segment_alignment: 8,
            group_segment_fixed_size: 0,
            include_export: true,
            include_canonical_descriptor_section_name: false,
        }
    }

    fn production_v1() -> Self {
        Self {
            required_workgroup_size: [64, 1, 1],
            max_flat_workgroup_size: 64,
            ..Self::valid()
        }
    }
}

#[test]
fn canonical_descriptor_section_presence_never_implies_finalization() {
    let fixture = fixture(FixtureOptions {
        include_canonical_descriptor_section_name: true,
        ..FixtureOptions::valid()
    });
    let inspected =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "vecadd", "vecadd.kd")).unwrap();

    assert_eq!(
        inspected.canonical_descriptor_section(),
        CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection
    );
    assert!(!inspected.canonical_descriptor_finalization_ran());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());
}

struct Fixture {
    bytes: Vec<u8>,
    descriptor_offset: usize,
    text_offset: usize,
}

#[test]
fn consumes_sealed_evidence_and_returns_inert_raw_inspection() {
    let fixture = fixture(FixtureOptions::valid());
    let original = fixture.bytes.clone();
    let evidence = evidence(fixture.bytes, "vecadd", "vecadd.kd");
    let attempt = evidence.attempt();
    let handoff = evidence.handoff_identity();
    let worker = evidence.worker_measurement().clone();
    let compiler_envelope = evidence.compiler_envelope_identity();
    let sealed_compiler_envelope = evidence
        .authorized()
        .response()
        .compiler_envelope_identity();
    let linked_output = evidence.output_identity();
    let request_id = *evidence.authorized().response().request_id();
    let request_identity = *evidence.authorized().response().request_identity();

    let inspected = inspect_worker_v2_raw_hsaco_v1(evidence).unwrap();
    assert_eq!(inspected.attempt(), attempt);
    assert_eq!(inspected.handoff_identity(), handoff);
    assert_eq!(inspected.worker_measurement(), &worker);
    assert_eq!(inspected.compiler_envelope_identity(), compiler_envelope);
    assert_eq!(
        inspected.sealed_compiler_envelope_identity(),
        sealed_compiler_envelope
    );
    assert_eq!(
        inspected.policy().compiler_envelope_identity(),
        compiler_envelope
    );
    assert_eq!(inspected.sealed_request_id(), &request_id);
    assert_eq!(inspected.sealed_request_identity(), &request_identity);
    assert_eq!(inspected.linked_output_identity(), linked_output);
    assert_eq!(inspected.exact_bytes(), original);
    assert!(linked_output.matches(inspected.exact_bytes()));
    assert_eq!(inspected.target().to_string(), TARGET);
    assert_eq!(
        inspected.canonical_descriptor_section(),
        CanonicalDescriptorSectionObservationV1::Missing
    );
    assert!(!inspected.canonical_descriptor_finalization_ran());
    assert!(!inspected.authenticates_compiler_origin());
    assert!(!inspected.policy().authenticates_compiler_origin());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());
    assert_ne!(inspected.identity().as_bytes(), &[0; 32]);
    assert_ne!(inspected.response_identity().as_bytes(), &[0; 32]);
    assert_ne!(inspected.policy().identity().as_bytes(), &[0; 32]);
}

#[test]
fn rejects_a_coherent_bare_gfx942_plan_before_hsaco_parsing() {
    let fixture = fixture(FixtureOptions {
        target: "gfx942",
        ..FixtureOptions::valid()
    });
    let error = inspect_worker_v2_raw_hsaco_v1(evidence_for_target(
        fixture.bytes,
        "vecadd",
        "vecadd.kd",
        "gfx942",
    ))
    .unwrap_err();
    assert_eq!(
        error,
        WorkerV2RawHsacoInspectionError::UnsupportedTarget("gfx942".to_owned())
    );
}

#[test]
fn rejects_artifact_target_and_code_object_version_mismatches() {
    let target = fixture(FixtureOptions {
        target: "gfx950",
        ..FixtureOptions::valid()
    });
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(target.bytes, "vecadd", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::TargetMismatch { .. })
    ));

    let version = fixture(FixtureOptions {
        code_object_version: 3,
        ..FixtureOptions::valid()
    });
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(version.bytes, "vecadd", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::CodeObjectVersionMismatch { .. })
    ));
}

#[test]
fn rejects_manifest_kernel_descriptor_and_export_closure_mismatches() {
    let bytes = fixture(FixtureOptions::valid()).bytes;
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(bytes.clone(), "other", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::KernelEntryRoleMismatch)
    ));
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(bytes, "vecadd", "other.kd")),
        Err(WorkerV2RawHsacoInspectionError::KernelDescriptorRoleMismatch)
    ));

    let no_export = fixture(FixtureOptions {
        include_export: false,
        ..FixtureOptions::valid()
    });
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(no_export.bytes, "vecadd", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::DefinedSymbolClosureMismatch { .. })
    ));
}

#[test]
fn rejects_required_size_max_flat_and_wavefront_launch_mismatches() {
    let wrong_required = fixture(FixtureOptions {
        required_workgroup_size: [128, 2, 1],
        ..FixtureOptions::valid()
    });
    let wrong_required =
        inspect_worker_v2_raw_hsaco_v1(evidence(wrong_required.bytes, "vecadd", "vecadd.kd"))
            .unwrap_err();
    assert_eq!(
        wrong_required,
        WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch {
            kernel: "vecadd".to_owned(),
            actual: Some([128, 2, 1]),
        }
    );
    assert_eq!(
        wrong_required.to_string(),
        "kernel vecadd requires Some([128, 2, 1]), expected [256, 1, 1]"
    );

    let wrong_max = fixture(FixtureOptions {
        max_flat_workgroup_size: 512,
        ..FixtureOptions::valid()
    });
    let wrong_max =
        inspect_worker_v2_raw_hsaco_v1(evidence(wrong_max.bytes, "vecadd", "vecadd.kd"))
            .unwrap_err();
    assert_eq!(
        wrong_max,
        WorkerV2RawHsacoInspectionError::MaxFlatWorkgroupSizeMismatch {
            kernel: "vecadd".to_owned(),
            actual: 512,
        }
    );
    assert_eq!(
        wrong_max.to_string(),
        "kernel vecadd max flat workgroup is 512, expected 256"
    );

    let wave32 = fixture(FixtureOptions {
        wavefront_size: 32,
        descriptor_wavefront_size: 32,
        ..FixtureOptions::valid()
    });
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(wave32.bytes, "vecadd", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::HsacoBinding(_))
            | Err(WorkerV2RawHsacoInspectionError::MetadataWavefrontSizeMismatch { .. })
    ));
}

#[test]
fn descriptor_and_elf_tampering_never_produces_raw_inspection_evidence() {
    let fixture = fixture(FixtureOptions::valid());

    let mut entry_offset = fixture.bytes.clone();
    entry_offset[fixture.descriptor_offset + 16] ^= 1;
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(entry_offset, "vecadd", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::HsacoBinding(_))
    ));

    let mut target_flags = fixture.bytes.clone();
    write_u32(&mut target_flags, 48, 0x54f);
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(target_flags, "vecadd", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::HsacoBinding(_))
    ));

    let mut truncated = fixture.bytes.clone();
    truncated.truncate(fixture.text_offset + 1);
    assert!(matches!(
        inspect_worker_v2_raw_hsaco_v1(evidence(truncated, "vecadd", "vecadd.kd")),
        Err(WorkerV2RawHsacoInspectionError::HsacoBinding(_))
    ));
}

#[test]
fn executable_byte_changes_are_bound_to_distinct_evidence_identities() {
    let fixture = fixture(FixtureOptions::valid());
    let original =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes.clone(), "vecadd", "vecadd.kd"))
            .unwrap();
    let mut changed = fixture.bytes;
    changed[fixture.text_offset] ^= 1;
    let changed = inspect_worker_v2_raw_hsaco_v1(evidence(changed, "vecadd", "vecadd.kd")).unwrap();

    assert_ne!(
        original.linked_output_identity(),
        changed.linked_output_identity()
    );
    assert_ne!(original.identity(), changed.identity());
    assert_ne!(original.exact_bytes(), changed.exact_bytes());
}

#[test]
fn protected_inspection_retains_exact_v2_closure_lineage_and_restart_inputs() {
    let fixture = fixture(FixtureOptions::valid());
    let original = fixture.bytes.clone();
    let closure = compiler_closure(0x20);
    let evidence = protected_evidence(fixture.bytes, "vecadd", "vecadd.kd", closure);
    let upstream = evidence.identity();
    let attempt = evidence.attempt();
    let handoff = evidence.handoff_identity();
    let plan = evidence.link_plan_identity();

    let inspected = inspect_protected_worker_v2_raw_hsaco_v1(evidence).unwrap();
    require_v2_handoff_identity(inspected.handoff_identity());
    assert_eq!(inspected.source_evidence_identity(), upstream);
    assert_eq!(inspected.upstream_evidence_identity(), upstream);
    assert_eq!(inspected.attempt(), attempt);
    assert_eq!(inspected.handoff_identity(), handoff);
    assert_eq!(inspected.compiler_closure(), closure);
    assert_eq!(inspected.plan().identity(), plan);
    assert_eq!(inspected.link_plan_identity(), plan);
    assert_eq!(inspected.exact_bytes(), original);
    assert!(
        inspected
            .linked_output_identity()
            .matches(inspected.exact_bytes())
    );
    assert!(!inspected.canonical_descriptor_finalization_ran());
    assert!(!inspected.authenticates_compiler_origin());
    assert!(!inspected.grants_compiler_authority());
    assert!(!inspected.grants_link_authority());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());
}

#[test]
fn protected_and_v1_inspection_schemas_remain_side_by_side_without_downgrade() {
    let fixture = fixture(FixtureOptions::valid());
    let ordinary =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes.clone(), "vecadd", "vecadd.kd"))
            .unwrap();
    let closure = compiler_closure(0x30);
    let protected = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        "vecadd",
        "vecadd.kd",
        closure,
    ))
    .unwrap();

    require_v2_handoff_identity(protected.handoff_identity());
    assert_eq!(protected.compiler_closure(), closure);
    assert_eq!(ordinary.exact_bytes(), protected.exact_bytes());
    assert_eq!(ordinary.policy(), protected.policy());
    assert_ne!(
        ordinary.identity().as_bytes(),
        protected.identity().as_bytes()
    );
}

#[test]
fn protected_production_v1_route_is_closed_over_its_wave64_contract() {
    let generic_fixture = fixture(FixtureOptions::production_v1());
    assert!(matches!(
        inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
            generic_fixture.bytes,
            "vecadd",
            "vecadd.kd",
            compiler_closure(0x40),
        )),
        Err(WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch { .. })
    ));

    let production_fixture = fixture(FixtureOptions::production_v1());
    let inspected = inspect_protected_production_v1_worker_v2_raw_hsaco_v1(protected_evidence(
        production_fixture.bytes,
        "vecadd",
        "vecadd.kd",
        compiler_closure(0x40),
    ))
    .unwrap();
    assert_eq!(
        inspected.policy().launch().required_workgroup_size(),
        [64, 1, 1]
    );
    assert_eq!(inspected.policy().launch().max_flat_workgroup_size(), 64);
    assert_eq!(inspected.policy().launch().wavefront_size(), 64);
}

#[test]
fn closure_and_valid_abi_resource_mutations_change_protected_inspection_identity() {
    let original = fixture(FixtureOptions::valid());
    let first = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        original.bytes.clone(),
        "vecadd",
        "vecadd.kd",
        compiler_closure(0x50),
    ))
    .unwrap();
    let changed_closure = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        original.bytes,
        "vecadd",
        "vecadd.kd",
        compiler_closure(0x60),
    ))
    .unwrap();
    let changed_abi = fixture(FixtureOptions {
        kernarg_segment_alignment: 16,
        ..FixtureOptions::valid()
    });
    let changed_abi = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        changed_abi.bytes,
        "vecadd",
        "vecadd.kd",
        compiler_closure(0x50),
    ))
    .unwrap();
    let changed_resources = fixture(FixtureOptions {
        group_segment_fixed_size: 64,
        ..FixtureOptions::valid()
    });
    let changed_resources = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        changed_resources.bytes,
        "vecadd",
        "vecadd.kd",
        compiler_closure(0x50),
    ))
    .unwrap();

    assert_eq!(first.exact_bytes(), changed_closure.exact_bytes());
    assert_ne!(first.compiler_closure(), changed_closure.compiler_closure());
    assert_ne!(first.identity(), changed_closure.identity());
    assert_ne!(first.identity(), changed_abi.identity());
    assert_ne!(first.identity(), changed_resources.identity());
}

#[test]
fn protected_inspection_rejects_descriptor_and_target_mutations() {
    let valid_fixture = fixture(FixtureOptions::valid());
    let descriptor_offset = valid_fixture.descriptor_offset;
    let mut descriptor = valid_fixture.bytes;
    descriptor[descriptor_offset + 16] ^= 1;
    assert!(matches!(
        inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
            descriptor,
            "vecadd",
            "vecadd.kd",
            compiler_closure(0x70),
        )),
        Err(WorkerV2RawHsacoInspectionError::HsacoBinding(_))
    ));

    let target = fixture(FixtureOptions {
        target: "gfx950",
        ..FixtureOptions::valid()
    });
    assert!(matches!(
        inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
            target.bytes,
            "vecadd",
            "vecadd.kd",
            compiler_closure(0x70),
        )),
        Err(WorkerV2RawHsacoInspectionError::TargetMismatch { .. })
    ));
}

fn require_v2_handoff_identity(_: CompilerModuleHandoffIdentityV2) {}

fn compiler_closure(seed: u8) -> CompilerClosureV2 {
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

fn evidence(
    bytes: Vec<u8>,
    manifest_entry: &str,
    manifest_descriptor: &str,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    evidence_for_target(bytes, manifest_entry, manifest_descriptor, TARGET)
}

fn evidence_for_target(
    bytes: Vec<u8>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    target: &str,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    let directory = TestDirectory::new();
    let producer = ProducerIdentity::from_codegen(
        "worker_v2_hsaco_admission_fixture",
        Some(Path::new("tests/worker_v2_hsaco_admission.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x91; 32]),
        BuildSession::from_bytes([0x92; 16]),
    )
    .unwrap();
    let handoff = compiler_handoff_for_target(&bytes, manifest_entry, manifest_descriptor, target);
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

fn protected_evidence(
    bytes: Vec<u8>,
    manifest_entry: &str,
    manifest_descriptor: &str,
    closure: CompilerClosureV2,
) -> fe2o3_hsaco_finalize::InertProtectedFirstBuildWorkerV2EvidenceV1 {
    let directory = TestDirectory::new();
    let producer = ProducerIdentity::from_codegen(
        "protected_worker_v2_hsaco_admission_fixture",
        Some(Path::new("tests/worker_v2_hsaco_admission.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0xa1; 32]),
        BuildSession::from_bytes([0xa2; 16]),
    )
    .unwrap();
    let handoff = compiler_handoff(&bytes, manifest_entry, manifest_descriptor);
    publish_compiler_module_handoff_v2(
        &directory.0,
        &producer,
        attempt,
        closure,
        handoff.canonical_bytes(),
    )
    .unwrap();
    let consumed =
        consume_compiler_module_handoff_v2(&directory.0, &producer, attempt, closure).unwrap();
    execute_protected_reproducible_first_build_worker_v2(
        consumed,
        &pinned_worker(),
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(64 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(2), 16 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap()
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v2-hsaco-admission-{}-{}",
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

fn compiler_handoff(
    bytes: &[u8],
    manifest_entry: &str,
    manifest_descriptor: &str,
) -> CompilerModuleHandoffV2 {
    compiler_handoff_for_target(bytes, manifest_entry, manifest_descriptor, TARGET)
}

fn compiler_handoff_for_target(
    bytes: &[u8],
    manifest_entry: &str,
    manifest_descriptor: &str,
    target: &str,
) -> CompilerModuleHandoffV2 {
    const PAYLOAD_MARKER: &[u8] = b"FE2O3/TEST-HSACO-PAYLOAD/V1\0";
    let parsed_target = CompilerDeviceTargetV1::parse(target).unwrap();
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
        CompilerFfiEnvelopeBuilderV1::new(parsed_target, CompilerCodeObjectVersion::V6, 1).unwrap();
    envelope.push(compiler_contract_for_target(target)).unwrap();
    let mut module = PAYLOAD_MARKER.to_vec();
    module.extend_from_slice(bytes);
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmBitcode,
        parsed_target,
        CompilerCodeObjectVersion::V6,
        envelope.finish().unwrap(),
        manifest,
        &module,
    )
    .unwrap()
}

fn compiler_contract_for_target(target: &str) -> CompilerFfiContractV1 {
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    let semantic_identity = [0x53; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "ffi_export",
        calling_convention: "C",
        code_object_version: 6,
        target,
        physical_abi: ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    };
    CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Export,
        CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        CompilerDeviceTargetV1::parse(target).unwrap(),
        CompilerCodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            "admission_fixture",
            "admission_fixture::ffi_export",
            [0x35; 16],
            "_RINvNtCs1234_17admission_fixture10ffi_export",
        )
        .unwrap(),
        "ffi_export",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}

fn fixture(options: FixtureOptions<'_>) -> Fixture {
    const PROGRAM_COUNT: usize = 2;
    let metadata = metadata(options);
    let note = metadata_note(&metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];

    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let descriptor_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0);
    let rodata_end = bytes.len();

    align(&mut bytes, 256);
    let text_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0xbf);
    let export_offset = if options.include_export {
        align(&mut bytes, 256);
        let offset = bytes.len();
        bytes.resize(bytes.len() + 64, 0xbe);
        Some(offset)
    } else {
        None
    };
    let text_end = bytes.len();

    let mut strtab = vec![0];
    let entry_name = push_name(&mut strtab, options.entry);
    let descriptor_name = push_name(&mut strtab, options.descriptor);
    let export_name = options
        .include_export
        .then(|| push_name(&mut strtab, "ffi_export"));
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);

    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 3 + usize::from(options.include_export);
    bytes.resize(symtab_offset + symbol_count * 24, 0);
    let entry_symbol = symtab_offset + 24;
    write_u32(&mut bytes, entry_symbol, entry_name);
    bytes[entry_symbol + 4] = 0x12;
    bytes[entry_symbol + 5] = 3;
    write_u16(&mut bytes, entry_symbol + 6, TEXT_SECTION_INDEX as u16);
    let entry_address = (text_offset + 0x1000) as u64;
    write_u64(&mut bytes, entry_symbol + 8, entry_address);
    write_u64(&mut bytes, entry_symbol + 16, 64);

    let descriptor_symbol = symtab_offset + 48;
    write_u32(&mut bytes, descriptor_symbol, descriptor_name);
    bytes[descriptor_symbol + 4] = 0x11;
    write_u16(
        &mut bytes,
        descriptor_symbol + 6,
        RODATA_SECTION_INDEX as u16,
    );
    write_u64(&mut bytes, descriptor_symbol + 8, descriptor_offset as u64);
    write_u64(&mut bytes, descriptor_symbol + 16, 64);

    if let (Some(name), Some(offset)) = (export_name, export_offset) {
        let export_symbol = symtab_offset + 72;
        write_u32(&mut bytes, export_symbol, name);
        bytes[export_symbol + 4] = 0x12;
        write_u16(&mut bytes, export_symbol + 6, TEXT_SECTION_INDEX as u16);
        write_u64(&mut bytes, export_symbol + 8, (offset + 0x1000) as u64);
        write_u64(&mut bytes, export_symbol + 16, 64);
    }

    write_u32(
        &mut bytes,
        descriptor_offset,
        options.group_segment_fixed_size,
    );
    write_u32(&mut bytes, descriptor_offset + 8, 272);
    write_i64(
        &mut bytes,
        descriptor_offset + 16,
        i64::try_from(entry_address - descriptor_offset as u64).unwrap(),
    );
    write_u32(&mut bytes, descriptor_offset + 44, 1);
    write_u32(&mut bytes, descriptor_offset + 48, 0x00af_0081);
    write_u32(&mut bytes, descriptor_offset + 52, 0x1390);
    write_u16(
        &mut bytes,
        descriptor_offset + 56,
        if options.descriptor_wavefront_size == 32 {
            0x041e
        } else {
            0x001e
        },
    );

    let mut shstr = vec![0];
    let note_name = push_name(&mut shstr, ".note");
    let rodata_name = push_name(&mut shstr, ".rodata");
    let text_name = push_name(&mut shstr, ".text");
    let strtab_name = push_name(&mut shstr, ".strtab");
    let symtab_name = push_name(&mut shstr, ".symtab");
    let shstrtab_name = push_name(
        &mut shstr,
        if options.include_canonical_descriptor_section_name {
            DEVICE_DESCRIPTOR_SECTION_NAME
        } else {
            ".shstrtab"
        },
    );
    let shstrtab_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    align(&mut bytes, 8);
    let section_table_offset = bytes.len();
    bytes.resize(section_table_offset + 7 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = options.code_object_version;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u32(&mut bytes, 48, target_flags(options.target));
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, 7);
    write_u16(&mut bytes, 62, SHSTRTAB_SECTION_INDEX as u16);

    let rodata_program = ELF_HEADER_BYTES;
    write_u32(&mut bytes, rodata_program, 1);
    write_u32(&mut bytes, rodata_program + 4, 4);
    write_u64(&mut bytes, rodata_program + 32, rodata_end as u64);
    write_u64(&mut bytes, rodata_program + 40, rodata_end as u64);
    write_u64(&mut bytes, rodata_program + 48, 0x1000);

    let text_program = ELF_HEADER_BYTES + PROGRAM_HEADER_BYTES;
    write_u32(&mut bytes, text_program, 1);
    write_u32(&mut bytes, text_program + 4, 5);
    write_u64(&mut bytes, text_program + 8, text_offset as u64);
    write_u64(&mut bytes, text_program + 16, (text_offset + 0x1000) as u64);
    write_u64(
        &mut bytes,
        text_program + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(
        &mut bytes,
        text_program + 40,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, text_program + 48, 0x1000);

    let note_header = section_table_offset + NOTE_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, note_name);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);

    let rodata_header = section_table_offset + RODATA_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, rodata_header, rodata_name);
    write_u32(&mut bytes, rodata_header + 4, 1);
    write_u64(&mut bytes, rodata_header + 8, 2);
    write_u64(&mut bytes, rodata_header + 16, rodata_offset as u64);
    write_u64(&mut bytes, rodata_header + 24, rodata_offset as u64);
    write_u64(
        &mut bytes,
        rodata_header + 32,
        (rodata_end - rodata_offset) as u64,
    );
    write_u64(&mut bytes, rodata_header + 48, 64);

    let text_header = section_table_offset + TEXT_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, text_header, text_name);
    write_u32(&mut bytes, text_header + 4, 1);
    write_u64(&mut bytes, text_header + 8, 6);
    write_u64(&mut bytes, text_header + 16, (text_offset + 0x1000) as u64);
    write_u64(&mut bytes, text_header + 24, text_offset as u64);
    write_u64(
        &mut bytes,
        text_header + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, text_header + 48, 256);

    let strtab_header = section_table_offset + STRTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strtab_header, strtab_name);
    write_u32(&mut bytes, strtab_header + 4, 3);
    write_u64(&mut bytes, strtab_header + 24, strtab_offset as u64);
    write_u64(&mut bytes, strtab_header + 32, strtab.len() as u64);
    write_u64(&mut bytes, strtab_header + 48, 1);

    let symtab_header = section_table_offset + SYMTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, symtab_header, symtab_name);
    write_u32(&mut bytes, symtab_header + 4, 2);
    write_u64(&mut bytes, symtab_header + 24, symtab_offset as u64);
    write_u64(&mut bytes, symtab_header + 32, (symbol_count * 24) as u64);
    write_u32(&mut bytes, symtab_header + 40, STRTAB_SECTION_INDEX as u32);
    write_u32(&mut bytes, symtab_header + 44, 1);
    write_u64(&mut bytes, symtab_header + 48, 8);
    write_u64(&mut bytes, symtab_header + 56, 24);

    let shstrtab_header = section_table_offset + SHSTRTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, shstrtab_header, shstrtab_name);
    write_u32(&mut bytes, shstrtab_header + 4, 3);
    write_u64(&mut bytes, shstrtab_header + 24, shstrtab_offset as u64);
    write_u64(&mut bytes, shstrtab_header + 32, shstr.len() as u64);
    write_u64(&mut bytes, shstrtab_header + 48, 1);

    Fixture {
        bytes,
        descriptor_offset,
        text_offset,
    }
}

fn metadata(options: FixtureOptions<'_>) -> Vec<u8> {
    let mut arguments = vec![
        argument(Some("values_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("values_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(v5_hidden_arguments(16));
    let kernel = Value::Map(vec![
        (Value::from(".name"), Value::from(options.entry)),
        (Value::from(".symbol"), Value::from(options.descriptor)),
        (Value::from(".args"), Value::Array(arguments)),
        (Value::from(".kernarg_segment_size"), Value::from(272)),
        (
            Value::from(".kernarg_segment_align"),
            Value::from(options.kernarg_segment_alignment),
        ),
        (
            Value::from(".group_segment_fixed_size"),
            Value::from(options.group_segment_fixed_size),
        ),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (
            Value::from(".wavefront_size"),
            Value::from(options.wavefront_size),
        ),
        (Value::from(".sgpr_count"), Value::from(14)),
        (Value::from(".vgpr_count"), Value::from(11)),
        (Value::from(".agpr_count"), Value::from(3)),
        (Value::from(".sgpr_spill_count"), Value::from(2)),
        (Value::from(".vgpr_spill_count"), Value::from(4)),
        (
            Value::from(".max_flat_workgroup_size"),
            Value::from(options.max_flat_workgroup_size),
        ),
        (
            Value::from(".reqd_workgroup_size"),
            Value::Array(
                options
                    .required_workgroup_size
                    .into_iter()
                    .map(Value::from)
                    .collect(),
            ),
        ),
    ]);
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from(format!("amdgcn-amd-amdhsa--{}", options.target)),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(vec![kernel])),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &root).unwrap();
    encoded
}

fn v5_hidden_arguments(base: u64) -> Vec<Value> {
    [
        (0, 4, "hidden_block_count_x"),
        (4, 4, "hidden_block_count_y"),
        (8, 4, "hidden_block_count_z"),
        (12, 2, "hidden_group_size_x"),
        (14, 2, "hidden_group_size_y"),
        (16, 2, "hidden_group_size_z"),
        (18, 2, "hidden_remainder_x"),
        (20, 2, "hidden_remainder_y"),
        (22, 2, "hidden_remainder_z"),
        (40, 8, "hidden_global_offset_x"),
        (48, 8, "hidden_global_offset_y"),
        (56, 8, "hidden_global_offset_z"),
        (64, 2, "hidden_grid_dims"),
    ]
    .into_iter()
    .map(|(offset, size, kind)| argument(None, base + offset, size, kind, None))
    .collect()
}

fn argument(
    name: Option<&str>,
    offset: u64,
    size: u64,
    value_kind: &str,
    address_space: Option<&str>,
) -> Value {
    let mut fields = vec![
        (Value::from(".offset"), Value::from(offset)),
        (Value::from(".size"), Value::from(size)),
        (Value::from(".value_kind"), Value::from(value_kind)),
    ];
    if let Some(name) = name {
        fields.push((Value::from(".name"), Value::from(name)));
    }
    if let Some(address_space) = address_space {
        fields.push((Value::from(".address_space"), Value::from(address_space)));
    }
    Value::Map(fields)
}

fn metadata_note(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&(owner.len() as u32).to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);
    note
}

fn target_flags(target: &str) -> u32 {
    match target {
        "gfx942" => 0x54c,
        "gfx942:xnack-" => 0x64c,
        "gfx950" => 0x54f,
        _ => panic!("unsupported test target"),
    }
}

fn push_name(strings: &mut Vec<u8>, name: &str) -> u32 {
    let offset = strings.len() as u32;
    strings.extend_from_slice(name.as_bytes());
    strings.push(0);
    offset
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let remainder = bytes.len() % alignment;
    if remainder != 0 {
        bytes.resize(bytes.len() + alignment - remainder, 0);
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut [u8], offset: usize, value: i64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
