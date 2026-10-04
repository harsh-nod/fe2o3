use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OUTPUT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestOutputDir {
    path: PathBuf,
    artifact_guard: PathBuf,
    artifact_guard_identity: String,
}

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let path = workspace.join(format!(
            "target/rustc-codegen-fe2o3-test-output/frontend-contract-{}-{}",
            std::process::id(),
            NEXT_OUTPUT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale frontend-contract output");
        }
        std::fs::create_dir_all(&path).expect("create frontend-contract output");
        let artifact_guard = path.join("artifact-path-guard");
        std::fs::create_dir(&artifact_guard).expect("create frontend-contract artifact guard");
        std::fs::set_permissions(&artifact_guard, std::fs::Permissions::from_mode(0o700))
            .expect("secure frontend-contract artifact guard");
        let metadata =
            std::fs::metadata(&artifact_guard).expect("inspect frontend-contract artifact guard");
        let artifact_guard_identity = format!("{:016x}:{:016x}", metadata.dev(), metadata.ino());
        Self {
            path,
            artifact_guard,
            artifact_guard_identity,
        }
    }

    fn configure_artifact_guard(&self, command: &mut Command) {
        command
            .env("FE2O3_ARTIFACT_PATH_GUARD_DIR", &self.artifact_guard)
            .env(
                "FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY",
                &self.artifact_guard_identity,
            );
    }
}

impl Drop for TestOutputDir {
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

fn fixtures(workspace: &Path) -> PathBuf {
    workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/frontend-contract")
}

fn rustc() -> Command {
    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
}

fn compile_frontend(source: &Path, output: &Path) -> Output {
    rustc()
        .arg(source)
        .args(["--edition=2024", "--crate-type=lib", "--emit=metadata"])
        .arg("-o")
        .arg(output)
        .output()
        .expect("compile frontend-contract fixture")
}

fn backend_build_command(workspace: &Path, target_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .arg("--target-dir")
        .arg(target_dir);
    command
}

fn backend(workspace: &Path, output: &TestOutputDir) -> PathBuf {
    let target_dir = output.path.join("cargo-target");
    let mut command = backend_build_command(workspace, &target_dir);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        command.arg("--release");
        "release"
    };
    let output = command.output().expect("build codegen backend");
    assert!(
        output.status.success(),
        "backend build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let dylib = target_dir.join(profile).join("librustc_codegen_fe2o3.so");
    assert!(dylib.is_file(), "missing backend at {}", dylib.display());
    dylib
}

#[test]
fn backend_fixture_build_uses_an_isolated_target_directory() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace);
    let target_dir = output.path.join("cargo-target");
    let command = backend_build_command(&workspace, &target_dir);
    let args = command.get_args().collect::<Vec<_>>();

    assert!(target_dir.starts_with(&output.path));
    assert_ne!(target_dir, workspace.join("target"));
    assert!(args.windows(2).any(|window| {
        window[0] == std::ffi::OsStr::new("--target-dir") && window[1] == target_dir.as_os_str()
    }));
}

fn compile_with_backend(
    source: &Path,
    crate_name: &str,
    backend: &Path,
    output: &TestOutputDir,
) -> Output {
    let artifact_dir = output.path.join(format!("{crate_name}-artifacts"));
    std::fs::create_dir_all(&artifact_dir).expect("create fixture artifact directory");
    let mut command = rustc();
    output.configure_artifact_guard(&mut command);
    command
        .arg(source)
        .args(["--edition=2024", "--crate-type=lib", "--crate-name"])
        .arg(crate_name)
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg("-o")
        .arg(output.path.join(format!("lib{crate_name}.rlib")))
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_HSACO_DIR", artifact_dir)
        .output()
        .expect("compile frontend-contract fixture with backend")
}

#[test]
fn fixture_corpus_clears_the_standard_rust_frontend() {
    let workspace = workspace();
    let fixtures = fixtures(&workspace);
    let output = TestOutputDir::new(&workspace);
    for fixture in [
        "genuine.rs",
        "reachable-helper.rs",
        "effectful.rs",
        "undeclared-asm.rs",
        "spoofed-target.rs",
        "duplicate.rs",
        "operand-mismatch.rs",
        "option-mismatch.rs",
    ] {
        let compiled = compile_frontend(
            &fixtures.join(fixture),
            &output.path.join(format!("{fixture}.rmeta")),
        );
        assert!(
            compiled.status.success(),
            "fixture `{fixture}` failed before the backend boundary:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

#[test]
#[ignore = "requires the protected cargo-fe2o3 Worker V3 rustc invocation"]
fn collector_authenticates_exact_roots_and_reachable_helpers_and_rejects_adversaries() {
    let workspace = workspace();
    let fixtures = fixtures(&workspace);
    let output = TestOutputDir::new(&workspace);
    let backend = backend(&workspace, &output);

    for (fixture, crate_name, effectful) in [
        ("genuine.rs", "frontend_contract_genuine", false),
        (
            "reachable-helper.rs",
            "frontend_contract_reachable_helper",
            false,
        ),
        ("effectful.rs", "frontend_contract_effectful", true),
    ] {
        let compiled = compile_with_backend(&fixtures.join(fixture), crate_name, &backend, &output);
        let stderr = String::from_utf8_lossy(&compiled.stderr);
        assert!(
            !compiled.status.success(),
            "fixture `{fixture}` unexpectedly passed the missing assembly-lowering boundary"
        );
        assert!(
            stderr.contains("authenticated 1 kernel frontend contract(s)")
                && stderr.contains("1 reachable asm block(s)"),
            "fixture `{fixture}` did not authenticate its exact source contract:\n{stderr}"
        );
        assert!(
            stderr.contains("cannot enter kernel IR until AMDGPU inline-assembly lowering"),
            "fixture `{fixture}` missed the explicit lowering blocker:\n{stderr}"
        );
        assert!(
            stderr.contains(if effectful {
                "1 effectful declaration(s)"
            } else {
                "0 effectful declaration(s)"
            }),
            "fixture `{fixture}` lost its declared effect summary:\n{stderr}"
        );
    }

    for (fixture, crate_name, expected) in [
        (
            "undeclared-asm.rs",
            "frontend_contract_undeclared",
            "reaches inline assembly without an authenticated unsafe_asm frontend contract",
        ),
        (
            "spoofed-target.rs",
            "frontend_contract_spoofed",
            "is not the exact registered kernel function",
        ),
        (
            "duplicate.rs",
            "frontend_contract_duplicate",
            "duplicate frontend contract for kernel `duplicate`",
        ),
        (
            "operand-mismatch.rs",
            "frontend_contract_operand_mismatch",
            "operand declaration 0x2 disagrees with reachable MIR operands 0x1",
        ),
        (
            "option-mismatch.rs",
            "frontend_contract_option_mismatch",
            "option declaration 0x19 disagrees with reachable MIR options 0x11",
        ),
    ] {
        let compiled = compile_with_backend(&fixtures.join(fixture), crate_name, &backend, &output);
        let stderr = String::from_utf8_lossy(&compiled.stderr);
        assert!(
            !compiled.status.success(),
            "adversarial fixture `{fixture}` unexpectedly compiled"
        );
        assert!(
            stderr.contains(expected),
            "adversarial fixture `{fixture}` missed `{expected}`:\n{stderr}"
        );
    }
}
