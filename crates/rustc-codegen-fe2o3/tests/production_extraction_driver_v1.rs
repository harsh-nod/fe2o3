use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, fail_build_attempt,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
    finalize_inspected_worker_v2_hsaco_v1, inspect_production_v1_worker_v2_raw_hsaco_v1,
    verify_finalized,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PRODUCTION_FILL_CRATE_BINDING_V1: &str =
    "e312f9362d2c716c79f0ce963d229ea0b6dcaf8c7112a675182e764916b2839b";
const PRODUCTION_WORKER_ENV: &str = "FE2O3_PRODUCTION_V1_WORKER";
const PRODUCTION_WORKER_BUILD_ID_ENV: &str = "FE2O3_PRODUCTION_V1_WORKER_BUILD_ID";
const PRODUCTION_LLVM_BUILD_ID_ENV: &str = "FE2O3_PRODUCTION_V1_LLVM_BUILD_ID";
const PRODUCTION_RAW_HSACO_ENV: &str = "FE2O3_PRODUCTION_V1_RAW_HSACO";
const PRODUCTION_FINALIZED_HSACO_ENV: &str = "FE2O3_PRODUCTION_V1_FINALIZED_HSACO";

struct ScratchTarget {
    path: PathBuf,
}

impl ScratchTarget {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-extraction-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create extraction target directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchTarget {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn production_backend() -> PathBuf {
    let backend = Path::new(env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
        .parent()
        .expect("extractor binary directory")
        .join(format!(
            "{}rustc_codegen_fe2o3{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX,
        ));
    assert!(backend.is_file(), "missing backend {}", backend.display());
    backend
}

fn materialize_source_safety_fixture(target: &ScratchTarget, source: &str) -> PathBuf {
    let fixture = target.path().join("fixture");
    std::fs::create_dir_all(fixture.join("src")).expect("create source-safety fixture");
    let root = workspace();
    let manifest = format!(
        r#"[package]
name = "fe2o3-production-extraction-fixture"
version = "0.1.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
fe2o3-device = {{ path = "{}" }}

[target.'cfg(not(target_arch = "amdgpu"))'.dependencies]
fe2o3-host = {{ path = "{}" }}

[lib]
name = "fe2o3_production_source_safety_fixture"
path = "src/lib.rs"
"#,
        root.join("crates/fe2o3-device").display(),
        root.join("crates/fe2o3-host").display(),
    );
    std::fs::write(fixture.join("Cargo.toml"), manifest)
        .expect("write source-safety fixture manifest");
    std::fs::copy(root.join("Cargo.lock"), fixture.join("Cargo.lock"))
        .expect("copy pinned workspace lockfile into source-safety fixture");
    std::fs::write(fixture.join("src/lib.rs"), source).expect("write source-safety fixture source");
    fixture
}

fn production_fill_command(target: &ScratchTarget, artifacts: &Path, backend: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .env("FE2O3_CODEGEN_PIPELINE", "production-v1")
        .env("FE2O3_HSACO_DIR", artifacts)
        .env("FE2O3_TARGET", "gfx942")
        .env(
            "FE2O3_CRATE_BINDING_ID_V1",
            PRODUCTION_FILL_CRATE_BINDING_V1,
        )
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "rustc",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-extraction-fixture",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(target.path().join("cargo"))
        .args(["--", &format!("-Zcodegen-backend={}", backend.display())]);
    command
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn production_collector_rejects_reachable_unsafe_rust_with_rooted_diagnostics() {
    for (case, source, expected) in [
        (
            "reachable-unsafe-fn",
            include_str!("fixtures/production-source-safety-device/reachable_unsafe_fn.rs"),
            [
                "ordinary production kernel `unsafe_reachable` reaches unsafe function instance",
                "reachable call chain:",
                "unsafe_reachable",
                "safe_bridge_to_unsafe_leaf",
                "unsafe_leaf",
            ],
        ),
        (
            "local-unsafe-block",
            include_str!("fixtures/production-source-safety-device/local_unsafe_block.rs"),
            [
                "ordinary production kernel `unsafe_block_reachable` reaches a safe-signature local helper containing a user-provided unsafe block",
                "reachable call chain:",
                "unsafe_block_reachable",
                "local_unsafe_block",
                "src/lib.rs:",
            ],
        ),
        (
            "external-hir-gap",
            include_str!("fixtures/production-source-safety-device/external_hir_gap.rs"),
            [
                "ordinary production kernel `external_hir_gap` cannot authenticate the absence of user-provided unsafe blocks in external helper",
                "cross-crate HIR is unavailable",
                "optimized MIR does not retain unsafe-block syntax",
                "reachable call chain:",
                "core::slice::<impl [T]>::is_empty",
            ],
        ),
    ] {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, source);
        let output = Command::new(env!("CARGO"))
            .current_dir(fixture)
            .env(
                "RUSTC_WORKSPACE_WRAPPER",
                env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
            )
            .env(
                "FE2O3_EXTRACT_CRATE_V1",
                "fe2o3_production_source_safety_fixture",
            )
            .env(
                "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
                "1111111111111111111111111111111111111111111111111111111111111111",
            )
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
            )
            .args([
                "check",
                "--offline",
                "-Zbuild-std=core",
                "--target",
                "amdgcn-amd-amdhsa",
                "--target-dir",
            ])
            .arg(target.path().join("cargo"))
            .output()
            .expect("run production source-safety fixture");
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
        assert!(
            !output.status.success(),
            "unsafe production fixture `{case}` unexpectedly compiled"
        );
        for expected in expected {
            assert!(
                stderr.contains(expected),
                "unsafe production fixture `{case}` omitted {expected:?}:\n{stderr}",
            );
        }
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn attributed_kernel_is_recollected_inside_a_real_amdgcn_dependency_graph() {
    let target = ScratchTarget::new();
    let repeated_target = ScratchTarget::new();
    let first = run_extraction(&target);
    let repeated = run_extraction(&repeated_target);

    assert_eq!(
        identity_inventory_sha256(&first),
        identity_inventory_sha256(&repeated),
        "separate AMD rustc processes derived different identity inventories",
    );
    assert_eq!(
        preflight_plan_sha256(&first),
        preflight_plan_sha256(&repeated),
        "separate AMD rustc processes derived different raw-MIR preflight plans",
    );
    assert_eq!(
        semantic_mir_sha256(&first),
        semantic_mir_sha256(&repeated),
        "separate AMD rustc processes admitted different canonical semantic MIR requests",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn production_fill_prepares_worker_handoff_before_requiring_managed_attempt() {
    let target = ScratchTarget::new();
    let artifacts = target.path().join("artifacts");
    std::fs::create_dir(&artifacts).expect("create production artifact directory");
    let output = production_fill_command(&target, &artifacts, &production_backend())
        .output()
        .expect("run production fill codegen route");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");

    assert!(
        !output.status.success(),
        "gfx942 mapping was unexpectedly enabled"
    );
    for expected in [
        "production-v1 lowered 1 admitted semantic function(s)",
        "verified target-neutral Kernel IR module",
        "with 6 exact block correspondence record(s)",
        "admitted complete formal memory obligations for a 2-invocation structural witness",
        "with 1 allocation(s), 1 access(es), 1 runtime bounds requirement(s), 0 runtime alias requirement(s), and 0 inter-invocation conflict(s)",
        "lowered exact target-bound KIR with compiler-selected-or-retained workgroup WorkgroupSize { x: 64, y: 1, z: 1 }",
        "deterministic gfx942:xnack- LLVM text",
        "artifact/launch authority false",
        "preparing exact Worker V2 handoff",
        "production-v1 Worker V2 handoff failed: kernel-ir-worker-v2 requires a managed FE2O3_BUILD_ATTEMPT_V1",
    ] {
        assert!(stderr.contains(expected), "missing {expected:?}:\n{stderr}");
    }
    for forbidden in [
        "production-v1 target-neutral lowering failed",
        "production-v1 formal memory admission failed",
        "production-v1 gfx942 target binding failed",
        "production-v1 gfx942 LLVM lowering failed",
        "legacy-v1",
        "kernel-ir-v1",
        "published inert",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "production fill entered forbidden path {forbidden:?}:\n{stderr}",
        );
    }
    assert!(
        std::fs::read_dir(&artifacts)
            .expect("read production artifact directory")
            .next()
            .is_none(),
        "target-neutral fill lowering emitted a production artifact",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn production_fill_publishes_exact_managed_worker_handoff() {
    let target = ScratchTarget::new();
    let artifacts = target.path().join("artifacts");
    std::fs::create_dir(&artifacts).expect("create production artifact directory");
    let source = Path::new(
        "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs",
    );
    let producer =
        ProducerIdentity::from_codegen("fe2o3_production_extraction_fixture", Some(source))
            .expect("production fixture producer");
    let attempt = begin_build_attempt(
        &artifacts,
        &producer,
        BuildInvocation::from_bytes([0x50; 32]),
        BuildSession::from_bytes([0x51; 16]),
    )
    .expect("begin managed production fixture attempt");
    let output = production_fill_command(&target, &artifacts, &production_backend())
        .env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value())
        .output()
        .expect("run managed production fill codegen route");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        output.status.success(),
        "managed production build failed:\n{stderr}"
    );
    assert!(
        stderr.contains("production-v1 published")
            && stderr.contains("inert exact gfx942:xnack- LLVM handoff")
    );

    let consumed = consume_compiler_module_handoff_v1(&artifacts, &producer, attempt)
        .expect("consume production compiler-module handoff");
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())
        .expect("decode canonical production handoff");
    assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(handoff.envelope().inspection().import_count(), 0);
    assert_eq!(handoff.envelope().inspection().export_count(), 0);
    assert_eq!(
        handoff
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
            .count(),
        1,
    );
    assert_eq!(
        handoff
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
            .count(),
        1,
    );
    let llvm = std::str::from_utf8(handoff.module_bytes()).expect("production LLVM is UTF-8");
    for required in [
        "define amdgpu_kernel",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "!{i32 64, i32 1, i32 1}",
        "module asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"",
    ] {
        assert!(llvm.contains(required), "missing {required:?}:\n{llvm}");
    }
    assert!(!handoff.grants_worker_authority());
    assert!(!handoff.grants_link_authority());
    assert!(!handoff.grants_load_authority());
    assert!(!handoff.grants_launch_authority());
    fail_build_attempt(&artifacts, &producer, attempt).expect("close production fixture attempt");
}

#[test]
#[ignore = "requires the measured upstream LLVM 22.1.8 worker and AMD target"]
fn production_fill_links_to_inspected_raw_hsaco_with_upstream_llvm() {
    let target = ScratchTarget::new();
    let artifacts = target.path().join("artifacts");
    std::fs::create_dir(&artifacts).expect("create production artifact directory");
    let source = Path::new(
        "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs",
    );
    let producer =
        ProducerIdentity::from_codegen("fe2o3_production_extraction_fixture", Some(source))
            .expect("production fixture producer");
    let attempt = begin_build_attempt(
        &artifacts,
        &producer,
        BuildInvocation::from_bytes([0x60; 32]),
        BuildSession::from_bytes([0x61; 16]),
    )
    .expect("begin managed production link attempt");
    let output = production_fill_command(&target, &artifacts, &production_backend())
        .env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value())
        .output()
        .expect("run managed production fill codegen route");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(output.status.success(), "production fill failed:\n{stderr}");

    let worker_path = PathBuf::from(required_env(PRODUCTION_WORKER_ENV));
    let worker_bytes = std::fs::read(&worker_path).expect("read measured production worker");
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&worker_bytes),
        required_env(PRODUCTION_WORKER_BUILD_ID_ENV),
        required_env(PRODUCTION_LLVM_BUILD_ID_ENV),
    )
    .expect("construct measured production worker identity");
    let worker = PinnedWorkerV1::open(&worker_path, measurement)
        .expect("capture measured production worker");
    let link_options = [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed production link option"))
    .collect();
    let consumed = consume_compiler_module_handoff_v1(&artifacts, &producer, attempt)
        .expect("consume production compiler-module handoff");
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed,
        &worker,
        Vec::new(),
        link_options,
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64)
            .expect("bounded production Worker output"),
        WorkerExecutionLimitsV1::default(),
    )
    .unwrap_or_else(|error| panic!("production upstream-LLVM Worker failed: {error:?}"));
    let diagnostics = evidence.authorized().response().diagnostics().to_vec();
    let inspected =
        inspect_production_v1_worker_v2_raw_hsaco_v1(evidence).unwrap_or_else(|error| {
            panic!("production raw-HSACO inspection failed: {error:?}; {diagnostics:?}")
        });
    assert_eq!(inspected.target().to_string(), "gfx942:xnack-");
    assert_eq!(
        inspected.code_object_version(),
        fe2o3_kernel_descriptor::CodeObjectVersion::V6
    );
    assert_eq!(
        inspected.policy().launch().required_workgroup_size(),
        [64, 1, 1]
    );
    assert_eq!(inspected.policy().launch().max_flat_workgroup_size(), 64);
    assert_eq!(inspected.policy().launch().wavefront_size(), 64);
    assert_eq!(
        inspected.policy().expected_defined_symbols(),
        ["fill", "fill.kd"]
    );
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());

    let raw_bytes = inspected.exact_bytes().to_vec();
    let finalized = finalize_inspected_worker_v2_hsaco_v1(inspected)
        .unwrap_or_else(|error| panic!("production canonical finalization failed: {error:?}"));
    assert_ne!(finalized.canonical_digest().as_bytes(), &[0; 32]);
    assert_ne!(finalized.exact_finalized_bytes(), raw_bytes);
    assert!(finalized.canonical_descriptor_finalization_ran());
    assert!(!finalized.grants_publication_authority());
    assert!(!finalized.grants_load_authority());
    assert!(!finalized.grants_launch_authority());
    let independently_verified = verify_finalized(finalized.exact_finalized_bytes())
        .expect("independently verify finalized production HSACO");
    assert_eq!(
        independently_verified.digest(),
        finalized.canonical_digest()
    );
    let [descriptor] = independently_verified.descriptor_table().kernels() else {
        panic!("finalized production HSACO does not contain one descriptor");
    };
    assert_eq!(descriptor.entry_name().as_str(), "fill");
    assert_eq!(descriptor.descriptor_symbol().as_str(), "fill.kd");
    assert_eq!(descriptor.abi_layout().explicit_argument_size(), 16);
    assert_eq!(descriptor.abi_layout().kernarg_segment_size(), 272);
    assert_eq!(descriptor.launch().max_flat_workgroup_size(), 64);

    let output_path = PathBuf::from(required_env(PRODUCTION_RAW_HSACO_ENV));
    let mut output_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)
        .expect("create fresh production raw HSACO");
    output_file
        .write_all(&raw_bytes)
        .expect("write production raw HSACO");
    output_file.sync_all().expect("sync production raw HSACO");
    let finalized_path = PathBuf::from(required_env(PRODUCTION_FINALIZED_HSACO_ENV));
    let mut finalized_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&finalized_path)
        .expect("create fresh production finalized HSACO");
    finalized_file
        .write_all(finalized.exact_finalized_bytes())
        .expect("write production finalized HSACO");
    finalized_file
        .sync_all()
        .expect("sync production finalized HSACO");
    fail_build_attempt(&artifacts, &producer, attempt).expect("close production link attempt");
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}

fn run_extraction(target: &ScratchTarget) -> String {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_extraction_fixture",
        )
        // The production cargo-fe2o3 parent owns this observation. This
        // process-isolation fixture has no production authority and supplies
        // only a nonzero test value so collection can reach the importer gate.
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-extraction-fixture",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(&target.path)
        .output()
        .expect("run AMD extraction fixture");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");

    assert!(
        !output.status.success(),
        "production pipeline unexpectedly passed the pending target-neutral lowering boundary"
    );
    let inventory_sha256 = identity_inventory_sha256(&stderr);
    let preflight_sha256 = preflight_plan_sha256(&stderr);
    let semantic_sha256 = semantic_mir_sha256(&stderr);
    let expected_milestone = format!(
        "production-v1 semantic importer authenticated rustc identity inventory {inventory_sha256} and bounded preflight plan {preflight_sha256}, then admitted one complete semantic MIR request with 1 function(s), 3 callable(s), and canonical identity {semantic_sha256}; an owner-held Pliron locator graph was recursively verified for exact semantic equivalence; target-neutral lowering remains pending; no fallback or artifact emission was entered",
    );
    assert!(
        stderr.contains(&expected_milestone),
        "missing exact admitted semantic MIR milestone diagnostic {expected_milestone:?}:\n{stderr}"
    );
    for forbidden in [
        "semantic import target rejection",
        "semantic importer rejected complete semantic MIR",
        "semantic importer rejected semantic body construction",
        "requires authoritative rustc LLVM target",
        "found no registered kernel",
        "body record construction remains pending",
        "schema-shaped semantic",
        "legacy-v1",
        "kernel-ir-v1",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "AMD extraction entered forbidden path {forbidden:?}:\n{stderr}"
        );
    }
    stderr
}

fn identity_inventory_sha256(stderr: &str) -> &str {
    canonical_sha256_after(
        stderr,
        "authenticated rustc identity inventory ",
        " and bounded preflight plan ",
        "rustc identity inventory",
    )
}

fn preflight_plan_sha256(stderr: &str) -> &str {
    canonical_sha256_after(
        stderr,
        "and bounded preflight plan ",
        ", then admitted one complete semantic MIR request with ",
        "rustc preflight plan",
    )
}

fn semantic_mir_sha256(stderr: &str) -> &str {
    canonical_sha256_after(
        stderr,
        "and canonical identity ",
        "; an owner-held Pliron locator graph was recursively verified for exact semantic equivalence; target-neutral lowering remains pending; no fallback or artifact emission was entered",
        "canonical semantic MIR",
    )
}

fn canonical_sha256_after<'a>(
    stderr: &'a str,
    prefix: &str,
    trailer: &str,
    label: &str,
) -> &'a str {
    assert_eq!(
        stderr.match_indices(prefix).count(),
        1,
        "expected exactly one {label} identity diagnostic:\n{stderr}",
    );
    let suffix = stderr
        .split_once(prefix)
        .unwrap_or_else(|| panic!("missing {label} identity diagnostic:\n{stderr}"))
        .1;
    let identity = suffix
        .get(..64)
        .unwrap_or_else(|| panic!("truncated {label} identity diagnostic:\n{stderr}"));
    assert!(
        identity
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} identity is not canonical lowercase hexadecimal: {identity:?}",
    );
    assert!(
        suffix[64..].starts_with(trailer),
        "{label} identity has a non-canonical diagnostic trailer:\n{stderr}",
    );
    identity
}
