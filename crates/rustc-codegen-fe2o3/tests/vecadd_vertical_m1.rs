use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest as _, Sha256};

static SCRATCH_NONCE: AtomicU64 = AtomicU64::new(0);

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-vecadd-m1-{label}-{}-{}",
            std::process::id(),
            SCRATCH_NONCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create fresh vecadd M1 scratch directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn fixture_manifest(workspace: &Path) -> PathBuf {
    workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/vecadd-capability-source/Cargo.toml")
}

fn check_fixture(workspace: &Path, target: &Path, target_kind: &str, name: &str) -> Output {
    Command::new(env!("CARGO"))
        .current_dir(workspace)
        .env_remove("CARGO_TARGET_DIR")
        .args(["check", "--locked", "--offline", "--manifest-path"])
        .arg(fixture_manifest(workspace))
        .args(["--target-dir"])
        .arg(target)
        .args([target_kind, name])
        .output()
        .expect("check vecadd capability fixture")
}

fn diagnostics(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

#[test]
fn ordinary_vecadd_capability_source_compiles() {
    let workspace = workspace();
    let scratch = ScratchDirectory::new("source");
    let output = check_fixture(&workspace, scratch.path(), "--bin", "valid");
    assert!(
        output.status.success(),
        "ordinary #[kernel(typed)] vecadd source did not compile:\n{}",
        diagnostics(&output),
    );
}

#[test]
fn named_source_capability_boundaries_fail_closed() {
    let workspace = workspace();
    let scratch = ScratchDirectory::new("source-negatives");
    let cases: &[(&str, &[&str])] = &[
        (
            "forged_context",
            &[
                "KernelContext",
                "no function or associated item named `default`",
            ],
        ),
        (
            "forged_global",
            &[
                "Global::new",
                "no associated function or constant named `new`",
            ],
        ),
        ("forged_view", &["Brand is compiler-bound"]),
        ("unchecked_bounds", &["cannot index into"]),
        ("wrong_read_role", &["no method named `load`"]),
        ("wrong_write_role", &["no method named `store`"]),
        ("aliased_output", &["cannot borrow `values` as mutable"]),
        ("wrong_kernel_abi", &["mismatched types"]),
    ];

    for &(name, expected) in cases {
        let output = check_fixture(&workspace, scratch.path(), "--example", name);
        let diagnostics = diagnostics(&output);
        assert!(
            !output.status.success(),
            "named boundary `{name}` unexpectedly compiled"
        );
        assert!(
            expected
                .iter()
                .any(|fragment| diagnostics.contains(fragment)),
            "named boundary `{name}` omitted its expected diagnostic {expected:?}:\n{diagnostics}",
        );
    }
}

fn remove_rustc_overrides(command: &mut Command) {
    for variable in [
        "RUSTC",
        "CARGO_BUILD_RUSTC",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(variable);
    }
}

fn cargo_fe2o3_executable(workspace: &Path, target: &Path) -> PathBuf {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace)
        .env_remove("CARGO_TARGET_DIR")
        .args(["build", "--locked", "--offline", "--target-dir"])
        .arg(target)
        .args(["-p", "cargo-fe2o3", "--bin", "cargo-fe2o3"]);
    remove_rustc_overrides(&mut command);
    let output = command.output().expect("build cargo-fe2o3");
    assert!(
        output.status.success(),
        "cargo-fe2o3 build failed:\n{}",
        diagnostics(&output),
    );
    let executable = target
        .join("debug")
        .join(format!("cargo-fe2o3{}", std::env::consts::EXE_SUFFIX));
    assert!(executable.is_file(), "missing {}", executable.display());
    executable
}

fn negative_production_config(workspace: &Path, target: &Path) -> PathBuf {
    let unreachable_worker = std::env::current_exe().expect("current M1 test executable");
    let bytes = fs::read(&unreachable_worker).expect("read inert unreachable worker");
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let config = target.join("vecadd-negative-production-config.json");
    let worker = unreachable_worker
        .to_str()
        .expect("UTF-8 M1 test executable path");
    let workspace = workspace.to_str().expect("UTF-8 workspace path");
    let json = format!(
        "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-production-build-config-v1\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"5\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"units\":[{{\"crate_name\":\"fe2o3_vecadd\",\"source\":\"examples/vecadd/src/main.rs\",\"working_directory\":{workspace:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":\"negative-test-unreachable-llvm\",\"path\":{worker:?},\"sha256\":\"{digest}\",\"worker_build_identity\":\"negative-test-unreachable-worker\"}}}}",
        bytes.len(),
    );
    fs::write(&config, json).expect("write negative production build config");
    config
}

fn vecadd_build(
    workspace: &Path,
    executable: &Path,
    target: &Path,
    build_config: &Path,
    device_target: &str,
) -> Output {
    let mut command = Command::new(executable);
    command
        .current_dir(workspace)
        .env("FE2O3_TARGET", device_target)
        .env("FE2O3_PRODUCTION_BUILD_CONFIG_V1", build_config)
        .env("CARGO_TARGET_DIR", target)
        .env_remove("LD_LIBRARY_PATH")
        .args(["build", "-p", "fe2o3-vecadd"]);
    remove_rustc_overrides(&mut command);
    command
        .output()
        .expect("run vecadd through the production cargo-fe2o3 entry")
}

#[test]
fn production_entry_rejects_unsealed_or_invalid_target_without_managed_output() {
    let workspace = workspace();
    let scratch = ScratchDirectory::new("production-negatives");
    let executable = cargo_fe2o3_executable(&workspace, scratch.path());
    let config = negative_production_config(&workspace, scratch.path());

    let rejected_output = vecadd_build(&workspace, &executable, scratch.path(), &config, "gfx000");
    assert!(
        !rejected_output.status.success(),
        "invalid target was accepted"
    );
    assert!(
        diagnostics(&rejected_output).contains(
            "production compilation requires exact FE2O3_TARGET=gfx942 or FE2O3_TARGET=gfx950"
        ),
        "invalid target omitted its fail-closed diagnostic:\n{}",
        diagnostics(&rejected_output),
    );
    assert!(
        !scratch.path().join("fe2o3").exists(),
        "a rejected build left fe2o3 outputs in a fresh directory",
    );

    let rejected_unsealed =
        vecadd_build(&workspace, &executable, scratch.path(), &config, "gfx942");
    assert!(
        !rejected_unsealed.status.success(),
        "production vecadd unexpectedly bypassed sealed release admission"
    );
    assert!(
        diagnostics(&rejected_unsealed).contains(
            "cargo fe2o3 authority release requires a protected pre-exec launcher/image contract; this build has no admitted release launcher"
        ),
        "unsealed production vecadd omitted its fail-closed diagnostic:\n{}",
        diagnostics(&rejected_unsealed),
    );
    assert!(
        !scratch.path().join("fe2o3").exists(),
        "a failed production transaction retained managed vecadd output",
    );
}

#[test]
fn vecadd_w6_integration_point_requires_owned_production_facts() {
    const HOST: &str = include_str!("../../../examples/vecadd/src/host_app.rs");
    const GENERATED_HOST: &str = include_str!("../../fe2o3-host/src/generated_host_contract_v2.rs");
    const GENERATED_KFD: &str = include_str!("../../fe2o3-host/src/generated_kfd_invocation.rs");
    for required in [
        "ProductionGeneratedHostFactsV2",
        "admit_generated_host_contract_v2",
        "AuthenticatedWorkerV3ExecutableV1",
        "prepare_direct_kfd_invocation_v2",
        "exact sealed V5 result",
        "machine-refinement receipt",
    ] {
        assert!(HOST.contains(required), "missing W6/W7 join `{required}`");
    }
    for forbidden in [
        "for_test_only",
        "from_canonical_bytes",
        "ProductionGeneratedHostFactsV2::from_production_capability_result_v5",
        "unsafe {",
    ] {
        assert!(
            !HOST.contains(forbidden),
            "vecadd fabricates authority via `{forbidden}`"
        );
    }
    for required in [
        "validate_sealed_v13_source_v2",
        "source.production_result_identity()",
        "source_result_identity.sha256() != result_identity.sha256()",
        "source_result_identity.byte_len() != result_identity.byte_len()",
    ] {
        assert!(
            GENERATED_HOST.contains(required),
            "generated-host admission omits sealed-result check `{required}`",
        );
    }
    for required in [
        "production_capability_result_sha256",
        "production_capability_result_bytes",
        "application_execution_identity_fails_closed_for_every_bound_coordinate",
        "assert_substitution_changes_identity!(production_capability_result_sha256",
        "assert_substitution_changes_identity!(production_capability_result_bytes",
    ] {
        assert!(
            GENERATED_KFD.contains(required),
            "direct-KFD preparation omits hostile substitution check `{required}`",
        );
    }
}
