use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_verifier::{
    validate_compiler_multi_root_proof_inputs_v1, validate_compiler_multi_root_target_lineage_v1,
};

#[path = "support/inert_invocation_v3.rs"]
mod inert_invocation_v3;

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
                "55".repeat(32),
            )
            .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
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
fn safe_scalar_from_bits_reaches_complete_semantic_import() {
    let target = ScratchTarget::new();
    let output = run_extraction_command(&target, Some("scalar-transmute"), true);
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");

    assert!(
        !output.status.success(),
        "scalar-transmute unexpectedly passed the pending target-neutral lowering boundary",
    );
    assert!(
        stderr.contains("then admitted one complete semantic MIR request")
            && stderr.contains("target-neutral lowering remains pending")
            && stderr.contains("no fallback or artifact emission was entered"),
        "safe f32::from_bits did not reach complete semantic import:\n{stderr}",
    );
    for forbidden in [
        "unsupported Transmute Cast rvalue",
        "semantic import target rejection",
        "semantic importer rejected complete semantic MIR",
        "semantic importer rejected semantic body construction",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "scalar-transmute entered forbidden path {forbidden:?}:\n{stderr}",
        );
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn rustc_fabs_f32_reaches_exact_llvm_intrinsic() {
    let target = ScratchTarget::new();
    let llvm_output = target.path().join("fabs-f32.ll");
    let output = run_llvm_extraction_command(&target, "fabs-f32", &llvm_output);
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        output.status.success(),
        "exact rustc fabs::<f32> extraction failed:\n{stderr}"
    );
    let llvm = std::fs::read_to_string(&llvm_output).expect("read fabs LLVM observation");
    assert!(
        llvm.contains("declare float @llvm.fabs.f32(float)")
            && llvm.contains("call float @llvm.fabs.f32(float")
            && llvm.contains("define amdgpu_kernel void @fabs_f32("),
        "fabs extraction omitted the exact target-neutral/LLVM observation:\n{llvm}",
    );
    assert_eq!(llvm.matches("call float @llvm.fabs.f32(float").count(), 1);
    assert!(!llvm.contains("@llvm.fabs.f64"));
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn core_atomic_rmw_set_reaches_complete_semantic_import() {
    let target = ScratchTarget::new();
    let output = run_extraction_command(&target, Some("atomic-rmw"), true);
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");

    assert!(
        !output.status.success(),
        "core atomics unexpectedly passed the pending target-neutral lowering boundary",
    );
    assert!(
        stderr.contains("then admitted one complete semantic MIR request")
            && stderr.contains("target-neutral lowering remains pending")
            && stderr.contains("no fallback or artifact emission was entered"),
        "core atomic RMWs did not reach complete semantic import:\n{stderr}",
    );
    for forbidden in [
        "reaches unsafe function instance",
        "cannot authenticate the absence of user-provided unsafe blocks",
        "unsupported rustc compiler intrinsic",
        "normalized atomic intrinsic with unexpected call arity",
        "semantic importer rejected complete semantic MIR",
        "semantic importer rejected semantic body construction",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "core atomic RMWs entered forbidden path {forbidden:?}:\n{stderr}",
        );
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn two_and_three_kernel_collections_reach_one_exact_multi_entry_llvm_module() {
    for (features, expected_symbols, expected_kir_version, expected_guarded_stores) in [
        ("multi-root-ownership", &["alpha", "zeta"][..], 8, 0),
        (
            "three-root-ownership",
            &["alpha", "omega", "zeta"][..],
            8,
            0,
        ),
        (
            "multi-root-ownership,write-only-disjoint-output",
            &["alpha", "fill_write_only_disjoint", "zeta"][..],
            9,
            1,
        ),
    ] {
        assert_multi_root_extraction(
            features,
            expected_symbols,
            expected_kir_version,
            expected_guarded_stores,
        );
    }
}

fn assert_multi_root_extraction(
    features: &str,
    expected_symbols: &[&str],
    expected_kir_version: u8,
    expected_guarded_stores: usize,
) {
    let target = ScratchTarget::new();
    let llvm_output = target.path().join("multi-root.ll");
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
        .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm_output)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "55".repeat(32),
        )
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
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
            "--features",
            features,
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(&target.path)
        .output()
        .expect("run multi-kernel AMD extraction fixture");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");

    assert!(
        output.status.success(),
        "{features} production extraction failed:\n{stderr}",
    );
    let expected_kir_custody = format!(
        "Kernel IR V{expected_kir_version} with {expected_guarded_stores} GuardedStore operation(s)"
    );
    assert!(
        stderr.contains(&expected_kir_custody)
            && stderr.contains("composed formal/ranked memory -> gfx942:xnack- LLVM")
            && stderr.contains("artifact/launch authority false"),
        "{features} extraction omitted its successful lowering receipt:\n{stderr}",
    );
    for forbidden in [
        "error[FE2O3-RACE",
        "lowering stopped",
        "panic",
        "MultiRootTargetNeutralLowering",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "{features} extraction emitted forbidden diagnostic {forbidden:?}:\n{stderr}",
        );
    }

    let llvm = std::fs::read_to_string(&llvm_output)
        .unwrap_or_else(|error| panic!("{features} did not emit LLVM: {error}"));
    assert_eq!(
        llvm.matches("define amdgpu_kernel void @").count(),
        expected_symbols.len(),
        "{features} LLVM did not contain exactly one kernel definition per root:\n{llvm}",
    );
    let mut offsets = Vec::new();
    for symbol in expected_symbols {
        let marker = format!("define amdgpu_kernel void @{symbol}(");
        assert_eq!(
            llvm.matches(&marker).count(),
            1,
            "{features} LLVM did not contain {symbol:?} exactly once:\n{llvm}",
        );
        offsets.push(llvm.find(&marker).unwrap());
    }
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "{features} LLVM changed canonical KernelId artifact order: {offsets:?}",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component, Verus runtime, and AMD target"]
fn proof_carrying_two_kernel_collection_reaches_exact_multi_root_target_lineage() {
    let target = ScratchTarget::new();
    let handoff_output = target.path().join("multi-root-semantic.handoff");
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
        .env(
            "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
            &handoff_output,
        )
        .env(
            "FE2O3_EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX",
            inert_invocation_v3::canonical_inert_gfx942_invocation_hex(),
        )
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "55".repeat(32),
        )
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
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
            "--features",
            "multi-root-target-lineage",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(&target.path)
        .output()
        .expect("run proof-carrying multi-kernel AMD extraction fixture");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        output.status.success()
            && stderr.contains("proof-carrying semantic compiler-bound inert handoff")
            && stderr.contains("artifact/launch authority false"),
        "proof-carrying multi-root production extraction failed:\n{stderr}",
    );

    let handoff_bytes = std::fs::read(&handoff_output).expect("read proof-carrying V3 handoff");
    let handoff = InertSemanticCompilerModuleHandoffV3::decode(&handoff_bytes)
        .expect("decode proof-carrying V3 handoff");
    let proof_inputs = validate_compiler_multi_root_proof_inputs_v1(
        handoff.capsule().receipts().proof_binding(),
        handoff.capsule().receipts().semantic_mir(),
        handoff.capsule().receipts().middle_end(),
        handoff.capsule().receipts().kernel_ir(),
        handoff.capsule().receipts().mir_to_kir_correspondence(),
        handoff.capsule().receipts().formal_memory(),
    )
    .expect("validate exact multi-root proof inputs");
    assert_eq!(proof_inputs.roots().len(), 2);
    let target_lineage =
        validate_compiler_multi_root_target_lineage_v1(handoff.capsule(), &proof_inputs)
            .expect("validate exact multi-root target lineage");
    assert!(target_lineage.has_exact_receipt_association());
    assert!(target_lineage.has_exact_kir_to_llvm_replay());

    let llvm = std::str::from_utf8(handoff.module_handoff().module_bytes())
        .expect("proof-carrying module is LLVM text");
    let alpha = llvm
        .find("define amdgpu_kernel void @alpha(")
        .expect("alpha kernel definition");
    let zeta = llvm
        .find("define amdgpu_kernel void @zeta(")
        .expect("zeta kernel definition");
    assert_eq!(llvm.matches("define amdgpu_kernel void @").count(), 2);
    assert!(
        alpha < zeta,
        "LLVM changed canonical KernelId artifact order"
    );
}

fn run_extraction(target: &ScratchTarget) -> String {
    let output = run_extraction_command(target, None, false);
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");

    assert!(
        !output.status.success(),
        "production pipeline unexpectedly passed the pending target-neutral lowering boundary"
    );
    let inventory_sha256 = identity_inventory_sha256(&stderr);
    let preflight_sha256 = preflight_plan_sha256(&stderr);
    let semantic_sha256 = semantic_mir_sha256(&stderr);
    let expected_milestone = format!(
        "production compilation semantic importer authenticated rustc identity inventory {inventory_sha256} and bounded preflight plan {preflight_sha256}, then admitted one complete semantic MIR request with 1 function(s), 3 callable(s), and canonical identity {semantic_sha256}; an owner-held Pliron locator graph was recursively verified for exact semantic equivalence; target-neutral lowering remains pending; no fallback or artifact emission was entered",
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
        "kernel-ir-v1",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "AMD extraction entered forbidden path {forbidden:?}:\n{stderr}"
        );
    }
    stderr
}

fn run_extraction_command(
    target: &ScratchTarget,
    features: Option<&str>,
    optimize: bool,
) -> std::process::Output {
    let target_rustflags = if optimize {
        "-Zalways-encode-mir -Copt-level=3 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"
    } else {
        "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"
    };
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_extraction_fixture",
        )
        // A caller-supplied observation has no authority. The selected
        // extractor must replace this stale value from exact rustc metadata.
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS", target_rustflags)
        .args([
            "check",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-extraction-fixture",
        ]);
    if let Some(features) = features {
        command.args(["--features", features]);
    }
    command
        .args(["--target", "amdgcn-amd-amdhsa", "--target-dir"])
        .arg(&target.path)
        .output()
        .expect("run AMD extraction fixture")
}

fn run_llvm_extraction_command(
    target: &ScratchTarget,
    feature: &str,
    llvm_output: &Path,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_extraction_fixture",
        )
        .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", llvm_output)
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Copt-level=3 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-extraction-fixture",
            "--features",
            feature,
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(target.path())
        .output()
        .expect("run AMD LLVM extraction fixture")
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
