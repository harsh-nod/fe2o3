use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

use fe2o3_artifact_transaction::{EmitError, ProducerIdentity, emit_artifact_transaction};
use fe2o3_artifacts::DigestAlgorithm;
use serde_json::json;

#[path = "support/cargo_fe2o3.rs"]
mod cargo_fe2o3;

const CONFIGURED_ARTIFACT_GUARD_CHILD_ENV: &str =
    "FE2O3_GENERAL_GEMM_CONFIGURED_ARTIFACT_GUARD_CHILD";

fn backend_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn cargo_target_directory(workspace: &Path) -> PathBuf {
    let Some(configured) = std::env::var_os("CARGO_TARGET_DIR") else {
        return workspace.join("target");
    };
    let configured = PathBuf::from(configured);
    if configured.is_absolute() {
        configured
    } else {
        workspace.join(configured)
    }
}

fn rerun_with_configured_artifact_path_guard(test_name: &str) -> bool {
    if std::env::var_os(CONFIGURED_ARTIFACT_GUARD_CHILD_ENV).is_some() {
        return false;
    }

    let workspace = workspace();
    let guard_directory = cargo_target_directory(&workspace).join(format!(
        "general-gemm-artifact-path-guard-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&guard_directory);
    std::fs::create_dir(&guard_directory).expect("create private general GEMM artifact path guard");
    std::fs::set_permissions(&guard_directory, std::fs::Permissions::from_mode(0o700))
        .expect("secure private general GEMM artifact path guard");
    let metadata = std::fs::metadata(&guard_directory)
        .expect("inspect private general GEMM artifact path guard");
    let identity = format!("{:016x}:{:016x}", metadata.dev(), metadata.ino());
    let child =
        Command::new(std::env::current_exe().expect("current general GEMM integration test"))
            .args(["--exact", test_name, "--nocapture"])
            .env(CONFIGURED_ARTIFACT_GUARD_CHILD_ENV, "1")
            .env("FE2O3_ARTIFACT_PATH_GUARD_DIR", &guard_directory)
            .env("FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY", identity)
            .output()
            .expect("run general GEMM test with a configured artifact path guard");
    let _ = std::fs::remove_dir_all(&guard_directory);
    assert!(
        child.status.success(),
        "configured artifact-path-guard general GEMM test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    true
}

fn fixture(workspace: &Path) -> PathBuf {
    workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/general-gemm-semantic-frontend")
}

fn managed_build(
    workspace: &Path,
    manifest: &Path,
    cargo_args: &[&str],
    artifacts: &Path,
) -> Output {
    Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "--locked",
            "--manifest-path",
        ])
        .arg(manifest)
        .args(cargo_args)
        .env(
            "FE2O3_BACKEND",
            cargo_target_directory(workspace).join("debug/librustc_codegen_fe2o3.so"),
        )
        .env("CARGO_TARGET_DIR", cargo_target_directory(workspace))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-v1")
        .env("FE2O3_HSACO_DIR", artifacts)
        .output()
        .expect("run managed general GEMM frontend build")
}

struct WorkerV2MissingEnvelope(PathBuf);

impl WorkerV2MissingEnvelope {
    fn new(workspace: &Path) -> Self {
        let worker = std::env::current_exe().expect("current test executable");
        let working_directory = workspace.join("examples/tiled_gemm_general_v1");
        let bytes = std::fs::read(&worker).expect("read current test executable");
        let digest = DigestAlgorithm::Sha256
            .calculate(&bytes)
            .bytes()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = cargo_target_directory(workspace).join(format!(
            "general-gemm-worker-v2-missing-envelope-{}.json",
            std::process::id()
        ));
        let config = json!({
            "candidate_output_max_bytes": 4_194_304,
            "format": "fe2o3-worker-v2-config-v2",
            "limits": {
                "stderr_bytes": 65_536,
                "stdout_bytes": 8_388_608,
                "timeout_ms": 30_000
            },
            "link_options": [
                {"name": "code-object-version", "value": "6"},
                {"name": "opt-level", "value": "2"},
                {"name": "strip-debug", "value": "true"},
                {"name": "verify-each", "value": "true"}
            ],
            "providers": [],
            "units": [{
                "crate_name": "fe2o3_tiled_gemm_general_v1",
                "source": "src/lib.rs",
                "working_directory": working_directory
            }],
            "worker": {
                "byte_len": bytes.len(),
                "llvm_build_identity": "test-only-unreached-llvm",
                "path": worker,
                "sha256": digest,
                "worker_build_identity": "test-only-unreached-worker"
            }
        });
        std::fs::write(
            &path,
            serde_json::to_vec(&config).expect("encode Worker V2 test config"),
        )
        .expect("write Worker V2 test config");
        Self(path)
    }
}

impl Drop for WorkerV2MissingEnvelope {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn managed_worker_v2_build(
    workspace: &Path,
    manifest: &Path,
    cargo_args: &[&str],
    artifacts: &Path,
    config: &Path,
) -> Output {
    cargo_fe2o3::non_production_command(workspace)
        .current_dir(workspace)
        .args(["build", "--locked", "--manifest-path"])
        .arg(manifest)
        .args(cargo_args)
        .env(
            "FE2O3_BACKEND",
            cargo_target_directory(workspace).join("debug/librustc_codegen_fe2o3.so"),
        )
        .env("CARGO_TARGET_DIR", cargo_target_directory(workspace))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_WORKER_V2_CONFIG_V2", config)
        .env("FE2O3_DUMP_MIR", "1")
        .env("FE2O3_HSACO_DIR", artifacts)
        .output()
        .expect("run managed Worker V2 general GEMM frontend build")
}

fn clear_artifacts(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn prepare_committed_generation(
    workspace: &Path,
    fixture: &Path,
    artifacts: &Path,
    source_bin: &str,
    kernel: &str,
) {
    clear_artifacts(artifacts);
    let initialized = managed_build(
        workspace,
        &workspace.join("Cargo.toml"),
        &["-p", "fe2o3-device"],
        artifacts,
    );
    assert!(
        initialized.status.success(),
        "failed to initialize the broker-owned artifact directory:\n{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let source = fixture.join(format!("src/bin/{source_bin}.rs"));
    let producer = ProducerIdentity::from_codegen(kernel, Some(&source))
        .expect("exact fixture producer identity");
    emit_artifact_transaction(
        artifacts,
        &producer,
        &[kernel],
        |name| *name,
        |_| Ok("must be transactionally removed".to_owned()),
        |llvm_ir, hsaco| {
            std::fs::write(hsaco.with_extension("o"), std::fs::read(llvm_ir)?)?;
            std::fs::write(hsaco, b"must be transactionally removed")?;
            Ok::<_, EmitError>(())
        },
    )
    .expect("commit an exactly owned stale artifact generation");
}

fn contains_regular_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.is_file() {
        return true;
    }
    if !metadata.is_dir() {
        return false;
    }
    std::fs::read_dir(path)
        .expect("read artifact directory")
        .any(|entry| contains_regular_file(&entry.expect("read artifact entry").path()))
}

fn assert_failed_without_artifact(output: &Output, artifacts: &Path) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "general GEMM frontend unexpectedly published an artifact:\n{stderr}"
    );
    assert!(
        !contains_regular_file(artifacts),
        "general GEMM frontend failure left an artifact in {}:\n{stderr}",
        artifacts.display()
    );
    stderr
}

#[test]
fn safe_general_gemm_mir_reaches_kir_and_exact_semantic_mutations_are_diagnostic() {
    if rerun_with_configured_artifact_path_guard(
        "safe_general_gemm_mir_reaches_kir_and_exact_semantic_mutations_are_diagnostic",
    ) {
        return;
    }
    let _lock = backend_test_lock();
    let workspace = workspace();
    let fixture = fixture(&workspace);
    let built = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build codegen backend");
    assert!(
        built.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    for source in [
        fixture.join("src/bin/valid_proof_sensitive.rs"),
        fixture.join("src/bin/missing_publish.rs"),
        fixture.join("src/bin/duplicate_store.rs"),
        fixture.join("src/bin/conditional_publish.rs"),
        fixture.join("src/bin/reversed_cycle.rs"),
        fixture.join("src/bin/store_loop.rs"),
        fixture.join("src/bin/incorrect_alpha_beta_epilogue.rs"),
    ] {
        let source = std::fs::read_to_string(&source).expect("read safe semantic fixture");
        assert!(source.contains("#![forbid(unsafe_code)]"));
        assert!(!source.contains("unsafe {"));
    }
    let baseline_source = std::fs::read_to_string(fixture.join("src/bin/valid_proof_sensitive.rs"))
        .expect("read full proof-sensitive baseline");
    let epilogue_mutation =
        std::fs::read_to_string(fixture.join("src/bin/incorrect_alpha_beta_epilogue.rs"))
            .expect("read full epilogue mutation");
    let restored_epilogue = epilogue_mutation.replacen(
        "let value = alpha * accumulator0 + initial;",
        "let value = alpha * accumulator0 + beta * initial;",
        1,
    );
    assert_eq!(
        restored_epilogue, baseline_source,
        "epilogue fixture must differ from the full baseline by exactly one named algebra mutation"
    );

    let impostor_fixture =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/general-gemm-provider-impostor");
    let brokered_artifacts = cargo_target_directory(&workspace).join("fe2o3");
    clear_artifacts(&brokered_artifacts);
    let impostor = managed_build(
        &workspace,
        &impostor_fixture.join("Cargo.toml"),
        &["-p", "general-gemm-provider-impostor-consumer"],
        &brokered_artifacts,
    );
    let impostor_stderr = assert_failed_without_artifact(&impostor, &brokered_artifacts);
    assert!(
        impostor_stderr.contains(
            "trusted-provider rejection: diagnostic item `fe2o3_device_general_tiled_gemm_proof_acquire_v1`"
        ) && impostor_stderr.contains(
            "not bound to the reviewed `fe2o3_gemm_device_v1` compilation unit"
        ) && impostor_stderr
            .contains("outside the reviewed fe2o3-device source root"),
        "same-name external general GEMM provider crossed the reviewed source boundary:\n{impostor_stderr}"
    );

    clear_artifacts(&brokered_artifacts);
    let worker_v2 = WorkerV2MissingEnvelope::new(&workspace);
    let positive = managed_worker_v2_build(
        &workspace,
        &workspace.join("examples/tiled_gemm_general_v1/Cargo.toml"),
        &["--release", "-p", "fe2o3-tiled-gemm-general-v1", "--lib"],
        &brokered_artifacts,
        &worker_v2.0,
    );
    let positive_stderr = assert_failed_without_artifact(&positive, &brokered_artifacts);
    assert!(
        !positive_stderr.contains("unsupported Assert terminator"),
        "positive safe source retained a MIR Assert terminator:\n{positive_stderr}"
    );
    let imported_mir = positive_stderr
        .find("=== fe2o3 MIR import scaffold")
        .unwrap_or_else(|| {
            panic!("positive safe source did not reach imported semantic MIR:\n{positive_stderr}")
        });
    let verified_kir = positive_stderr
        .find("selected kernel-ir-worker-v2: verified compiler-module candidate with 1 kernel(s)")
        .unwrap_or_else(|| {
            panic!("positive safe source did not reach verified Kernel IR:\n{positive_stderr}")
        });
    let missing_envelope = positive_stderr
        .find("Worker V2 producer failed: kernel-ir-worker-v2 requires a complete compiler FFI envelope")
        .unwrap_or_else(|| {
            panic!("positive safe source missed the deliberate post-KIR boundary:\n{positive_stderr}")
        });
    assert!(
        positive_stderr.contains("mir.load")
            && positive_stderr.contains("local1.deref")
            && positive_stderr.contains("local2.deref")
            && imported_mir < missing_envelope
            && verified_kir < missing_envelope,
        "checked A/B slice loads did not cross semantic MIR import and verified KIR construction:\n{positive_stderr}"
    );
    for unrelated in [
        "general typed V3 requires kernel-ir-worker-v2 shared publication",
        "publication state: NotStarted",
        "compiler FFI MIR import failed",
        "compiler-module MIR translation failed",
        "reached verified symbolic semantic template",
    ] {
        assert!(
            !positive_stderr.contains(unrelated),
            "positive safe source stopped at unrelated earlier failure {unrelated:?}:\n{positive_stderr}"
        );
    }
    assert!(
        !positive_stderr.contains("published inert Worker V2 compiler-module handoff"),
        "missing-envelope boundary unexpectedly published a Worker V2 handoff:\n{positive_stderr}"
    );

    prepare_committed_generation(
        &workspace,
        &fixture,
        &brokered_artifacts,
        "valid_proof_sensitive",
        "valid_proof_sensitive",
    );
    let baseline = managed_build(
        &workspace,
        &fixture.join("Cargo.toml"),
        &["--release", "--bin", "valid-proof-sensitive"],
        &brokered_artifacts,
    );
    let baseline_stderr = assert_failed_without_artifact(&baseline, &brokered_artifacts);
    assert!(
        baseline_stderr.contains(
            "authenticated general GEMM proof-sensitive mutation-oracle baseline passed its normalized MIR validators"
        ) && baseline_stderr.contains(
            "this source is non-executable and cannot issue frontend correspondence or artifact authority"
        ) && !baseline_stderr.contains("Unknown/Unproved")
            && !baseline_stderr.contains("reached verified symbolic semantic template"),
        "full proof-sensitive baseline did not cross authenticated MIR admission without gaining authority:\n{baseline_stderr}"
    );

    for (bin, root, code, property, stage) in [
        (
            "unguarded-a-tail-load",
            "valid_proof_sensitive",
            "0x46470102",
            "bounds_safe",
            "tile",
        ),
        (
            "unguarded-b-tail-load",
            "valid_proof_sensitive",
            "0x46470102",
            "bounds_safe",
            "tile",
        ),
        (
            "out-of-bounds-c-store",
            "valid_proof_sensitive",
            "0x46470102",
            "bounds_safe",
            "tile",
        ),
        (
            "lane-output-collision",
            "valid_proof_sensitive",
            "0x46470106",
            "output_region_injective",
            "tile",
        ),
        (
            "workgroup-output-collision",
            "valid_proof_sensitive",
            "0x46470106",
            "output_region_injective",
            "tile",
        ),
        (
            "lds-write-collision",
            "valid_proof_sensitive",
            "0x46470104",
            "race_free",
            "gpu",
        ),
        (
            "missing-b-stage-initialization",
            "valid_proof_sensitive",
            "0x46470103",
            "initialized",
            "gpu",
        ),
        (
            "missing-publish",
            "valid_proof_sensitive",
            "0x46470103",
            "initialized",
            "gpu",
        ),
        (
            "divergent-publish",
            "valid_proof_sensitive",
            "0x46470105",
            "barrier_convergent",
            "gpu",
        ),
        (
            "missing-reuse",
            "valid_proof_sensitive",
            "0x46470107",
            "lds_epoch_correct",
            "gpu",
        ),
        (
            "expired-lds-epoch",
            "valid_proof_sensitive",
            "0x46470107",
            "lds_epoch_correct",
            "gpu",
        ),
        (
            "read-before-wait",
            "valid_proof_sensitive",
            "0x46470103",
            "initialized",
            "gpu",
        ),
        (
            "reset-accumulator",
            "valid_proof_sensitive",
            "0x46470108",
            "accumulator_phase_refinement",
            "kernel",
        ),
        (
            "incorrect-k-tail-zero-fill",
            "valid_proof_sensitive",
            "0x46470109",
            "tail_refinement",
            "kernel",
        ),
        (
            "incorrect-alpha-beta-epilogue",
            "valid_proof_sensitive",
            "0x4647010a",
            "epilogue_refinement",
            "kernel",
        ),
        // This older reduced fixture is not one of the exact 15 reversible
        // corpus edits, but retains coverage for repeated sequential stores.
        (
            "duplicate-store",
            "duplicate_store",
            "0x46470106",
            "output_region_injective",
            "tile",
        ),
    ] {
        let source_bin = bin.replace('-', "_");
        prepare_committed_generation(&workspace, &fixture, &brokered_artifacts, &source_bin, root);
        let rejected = managed_build(
            &workspace,
            &fixture.join("Cargo.toml"),
            &["--release", "--bin", bin],
            &brokered_artifacts,
        );
        let stderr = assert_failed_without_artifact(&rejected, &brokered_artifacts);
        assert!(
            stderr.contains(&format!(
                "authenticated general GEMM semantic KIR rejected: general GEMM {property} counterexample at {stage}"
            )) && stderr.contains(code),
            "safe semantic fixture `{bin}` missed exact {code} diagnostic:\n{stderr}"
        );
        if let Some((failed, proven)) = match bin {
            "unguarded-a-tail-load" => Some((
                "A dimension 0 requires `row < m`",
                "A dimension 1 satisfies `depth < k`",
            )),
            "unguarded-b-tail-load" => Some((
                "B dimension 0 requires `depth < k`",
                "B dimension 1 satisfies `column < n`",
            )),
            "out-of-bounds-c-store" => Some((
                "C dimension 0 requires `row < m`",
                "C dimension 1 satisfies `column < n`",
            )),
            _ => None,
        } {
            assert!(
                stderr.contains(&format!("failed bound: {failed}"))
                    && stderr.contains(&format!("proven bound: {proven}"))
                    && stderr.contains(
                        "help: guard every path to the access with the failed relation or use a checked operation that supplies a defined tail value"
                    ),
                "safe semantic fixture `{bin}` omitted its dimension-specific bound assessment:\n{stderr}"
            );
        }
        assert!(
            !stderr.contains("reached verified symbolic semantic template"),
            "safe semantic fixture `{bin}` acquired a verified witness:\n{stderr}"
        );
        assert!(
            stderr.contains("kind=Counterexample")
                && stderr.contains(&format!("root symbol={root}"))
                && stderr.contains(&format!("source span=src/bin/{source_bin}.rs:"))
                && stderr.contains("terminal spans=")
                && stderr.contains("reachable call chain: kernel-root ->")
                && stderr.contains("no artifact authority was issued")
                && !stderr.contains("published inert Worker V2")
                && !stderr.contains("launch authority issued")
                && !stderr.contains("proof authority issued")
                && !stderr.contains("portable MIR identity mismatch")
                && !stderr.contains("Unknown/Unproved"),
            "safe semantic fixture `{bin}` diagnostic omitted its stable counterexample/root/span/call-chain receipt:\n{stderr}"
        );
    }

    for bin in ["conditional-publish", "reversed-cycle", "store-loop"] {
        clear_artifacts(&brokered_artifacts);
        let rejected = managed_build(
            &workspace,
            &fixture.join("Cargo.toml"),
            &["--release", "--bin", bin],
            &brokered_artifacts,
        );
        let stderr = assert_failed_without_artifact(&rejected, &brokered_artifacts);
        assert!(
            stderr.contains("general GEMM authenticated MIR import failed:")
                && stderr.contains("general GEMM semantic fact is Unknown/Unproved:"),
            "safe hostile CFG fixture `{bin}` missed fail-closed MIR admission:\n{stderr}"
        );
        assert!(
            !stderr.contains("reached verified symbolic semantic template"),
            "safe hostile CFG fixture `{bin}` acquired a verified witness:\n{stderr}"
        );
    }

    clear_artifacts(&brokered_artifacts);
}
