use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TestOutputDir(PathBuf);

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let path = workspace.join(format!(
            "target/rustc-codegen-fe2o3-test-output/cross-crate-import-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create output directory");
        Self(path)
    }
}

impl Drop for TestOutputDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn require_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn one_rlib(directory: &Path, prefix: &str) -> PathBuf {
    let mut matches = std::fs::read_dir(directory)
        .expect("read dependency directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|value| value.to_str()) == Some("rlib")
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    matches.sort();
    assert_eq!(matches.len(), 1, "expected one {prefix}*.rlib");
    matches.pop().unwrap()
}

fn build_backend(workspace: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build backend");
    require_success("backend build", &output);
    workspace.join("target/debug/librustc_codegen_fe2o3.so")
}

fn build_provider(workspace: &Path, output: &TestOutputDir) -> (PathBuf, PathBuf) {
    let manifest = workspace
        .join("crates/rustc-codegen-fe2o3/tests/fixtures/cross-crate-import/provider/Cargo.toml");
    let target = output.0.join("provider-target");
    let crate_binding = reserved_fe2o3_symbols::derive_crate_binding_id_v1(
        "fe2o3_cross_crate_provider",
        ["cross-crate-provider-v1"],
    );
    let build = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["rustc", "--locked", "--manifest-path"])
        .arg(&manifest)
        .args(["--lib", "--", "-Zalways-encode-mir"])
        .env("CARGO_TARGET_DIR", &target)
        .env("FE2O3_CRATE_BINDING_ID_V1", crate_binding.to_hex())
        .output()
        .expect("build provider");
    require_success("provider build", &build);

    let dependencies = target.join("debug/deps");
    (
        one_rlib(&dependencies, "libfe2o3_cross_crate_provider-"),
        one_rlib(&dependencies, "libfe2o3_device-"),
    )
}

fn run_consumer(
    workspace: &Path,
    output: &TestOutputDir,
    source: &Path,
    backend: &Path,
    provider: &Path,
    device: &Path,
    label: &str,
) -> Output {
    let dependencies = provider.parent().expect("provider dependency directory");
    Command::new("rustc")
        .current_dir(workspace)
        .arg(source)
        .args(["--edition=2024", "--crate-type=lib", "--emit=obj"])
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .args(["-Cpanic=abort", "-Cmetadata=cross-crate-consumer-v1"])
        .arg("--extern")
        .arg(format!("provider={}", provider.display()))
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("-o")
        .arg(output.0.join(format!("{label}.o")))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_VERBOSE", "1")
        .env(
            "FE2O3_HSACO_DIR",
            output.0.join(format!("{label}-artifacts")),
        )
        .env(
            "CARGO_MANIFEST_DIR",
            source
                .parent()
                .expect("consumer fixture directory")
                .join("consumer"),
        )
        .output()
        .expect("run consumer backend")
}

#[test]
#[ignore = "runs the configured rustc codegen backend"]
fn gfx942_imports_one_external_kernel_and_device_export_with_exact_identity() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace);
    let backend = build_backend(&workspace);
    let (provider, device) = build_provider(&workspace, &output);
    let fixtures = workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/cross-crate-import");

    let accepted = run_consumer(
        &workspace,
        &output,
        &fixtures.join("consumer.rs"),
        &backend,
        &provider,
        &device,
        "accepted",
    );
    require_success("cross-crate consumer", &accepted);
    let stderr = String::from_utf8_lossy(&accepted.stderr);
    assert!(
        stderr.contains("external_vecadd")
            && stderr.contains("fe2o3_cross_crate_provider")
            && stderr.contains("validated local device FFI evidence: 0 imports, 1 exports")
            && stderr.contains("collected compiler FFI envelope"),
        "missing exact cross-crate evidence\n{stderr}"
    );

    let rejected = run_consumer(
        &workspace,
        &output,
        &fixtures.join("consumer-substituted-anchor.rs"),
        &backend,
        &provider,
        &device,
        "substituted-anchor",
    );
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        !rejected.status.success(),
        "substituted anchor was accepted"
    );
    assert!(
        stderr.contains("FE2O3-FFI-XCR010")
            && stderr.contains("does not match exact function contract"),
        "missing fail-closed diagnostic\n{stderr}"
    );
}
