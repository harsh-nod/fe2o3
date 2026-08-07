use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OUTPUT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestOutputDir {
    path: PathBuf,
}

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let path = workspace.join(format!(
            "target/rustc-codegen-fe2o3-test-output/g2-monomorphization-{}-{}",
            std::process::id(),
            NEXT_OUTPUT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale test output");
        }
        std::fs::create_dir_all(&path).expect("create test output");
        Self { path }
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
    workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/g2-monomorphization")
}

fn rustc() -> Command {
    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
}

fn require_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn compile_rlib(source: &Path, crate_name: &str, output: &Path) {
    let result = rustc()
        .arg(source)
        .args(["--edition=2024", "--crate-type=rlib", "--crate-name"])
        .arg(crate_name)
        .arg("-o")
        .arg(output)
        .output()
        .expect("compile fixture rlib");
    require_success(crate_name, &result);
}

fn compile_frontend(source: &Path, output: &Path, externs: &[(&str, &Path)]) -> Output {
    let mut command = rustc();
    command
        .arg(source)
        .args(["--edition=2024", "--emit=metadata"])
        .arg("-o")
        .arg(output);
    for &(name, path) in externs {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    command.output().expect("compile frontend fixture")
}

fn build_backend(workspace: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build backend");
    require_success("backend build", &output);
    let backend = workspace.join("target/debug/librustc_codegen_fe2o3.so");
    assert!(
        backend.is_file(),
        "missing backend at {}",
        backend.display()
    );
    backend
}

fn compile_with_backend(
    source: &Path,
    crate_name: &str,
    backend: &Path,
    output_dir: &Path,
    externs: &[(&str, &Path)],
) -> Output {
    let mut command = rustc();
    command
        .arg(source)
        .args(["--edition=2024", "--crate-name", crate_name])
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg("-o")
        .arg(output_dir.join(crate_name))
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_HSACO_DIR", output_dir.join("artifacts"))
        .env(
            "FE2O3_TARGET",
            std::env::var("FE2O3_TEST_TARGET").unwrap_or_else(|_| "gfx1100".to_owned()),
        );
    for &(name, path) in externs {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    std::fs::create_dir_all(output_dir.join("artifacts")).expect("create artifact directory");
    command.output().expect("compile fixture with backend")
}

fn collection_rows(stderr: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut in_collection = false;
    for line in stderr.lines() {
        if line == "=== fe2o3 device function collection ===" {
            in_collection = true;
        } else if line == "========================================" {
            break;
        } else if in_collection
            && (line.trim_start().starts_with("path:")
                || line.trim_start().starts_with("instance:"))
        {
            rows.push(line.trim().to_owned());
        }
    }
    rows
}

#[test]
fn fixture_corpus_clears_the_standard_frontend_without_manifests() {
    let workspace = workspace();
    let fixtures = fixtures(&workspace);
    let output = TestOutputDir::new(&workspace);
    let shared_a = output.path.join("libg2_shared_a.rlib");
    let shared_b = output.path.join("libg2_shared_b.rlib");
    let unavailable = output.path.join("libg2_unavailable_helper.rlib");
    compile_rlib(&fixtures.join("shared-a.rs"), "g2_shared_a", &shared_a);
    compile_rlib(&fixtures.join("shared-b.rs"), "g2_shared_b", &shared_b);
    compile_rlib(
        &fixtures.join("unavailable-helper.rs"),
        "g2_unavailable_helper",
        &unavailable,
    );

    let collectible = compile_frontend(
        &fixtures.join("collectible.rs"),
        &output.path.join("collectible.rmeta"),
        &[("g2_shared_a", &shared_a), ("g2_shared_b", &shared_b)],
    );
    require_success("collectible frontend", &collectible);
    let unavailable_root = compile_frontend(
        &fixtures.join("unavailable.rs"),
        &output.path.join("unavailable.rmeta"),
        &[("g2_unavailable_helper", &unavailable)],
    );
    require_success("unavailable-MIR frontend", &unavailable_root);
    let malformed = compile_frontend(
        &fixtures.join("malformed-registration.rs"),
        &output.path.join("malformed.rmeta"),
        &[],
    );
    require_success("malformed registration frontend", &malformed);
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn collector_resolves_concrete_instances_and_rejects_unavailable_mir_stably() {
    let workspace = workspace();
    let fixtures = fixtures(&workspace);
    let output = TestOutputDir::new(&workspace);
    let backend = build_backend(&workspace);
    let shared_a = output.path.join("libg2_shared_a.rlib");
    let shared_b = output.path.join("libg2_shared_b.rlib");
    let unavailable = output.path.join("libg2_unavailable_helper.rlib");
    compile_rlib(&fixtures.join("shared-a.rs"), "g2_shared_a", &shared_a);
    compile_rlib(&fixtures.join("shared-b.rs"), "g2_shared_b", &shared_b);
    compile_rlib(
        &fixtures.join("unavailable-helper.rs"),
        "g2_unavailable_helper",
        &unavailable,
    );

    let externs = [
        ("g2_shared_a", shared_a.as_path()),
        ("g2_shared_b", shared_b.as_path()),
    ];
    let first = compile_with_backend(
        &fixtures.join("collectible.rs"),
        "g2_collectible",
        &backend,
        &output.path.join("first"),
        &externs,
    );
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    let rows = collection_rows(&first_stderr);
    assert!(
        !rows.is_empty(),
        "collector dump is missing:\n{first_stderr}"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.ends_with("generic_identity"))
            .count(),
        2,
        "two concrete generic types should be collected once each:\n{first_stderr}"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.ends_with("const_bias"))
            .count(),
        2,
        "two concrete const-generic instances should be collected:\n{first_stderr}"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.ends_with("recursive_sum"))
            .count(),
        1,
        "the recursive cycle should terminate at one concrete instance:\n{first_stderr}"
    );
    assert!(
        rows.iter().any(|row| row == "path: g2_shared_a::same_name")
            && rows.iter().any(|row| row == "path: g2_shared_b::same_name"),
        "same-name helpers from two crates were not both collected:\n{first_stderr}"
    );
    let identities = rows
        .iter()
        .filter(|row| row.starts_with("instance:"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        identities.len() * 2,
        rows.len(),
        "every collected path must have a distinct concrete identity:\n{first_stderr}"
    );

    let second = compile_with_backend(
        &fixtures.join("collectible.rs"),
        "g2_collectible",
        &backend,
        &output.path.join("second"),
        &externs,
    );
    let second_stderr = String::from_utf8_lossy(&second.stderr);
    assert_eq!(
        rows,
        collection_rows(&second_stderr),
        "collection order or identities changed"
    );

    let unavailable_result = compile_with_backend(
        &fixtures.join("unavailable.rs"),
        "g2_unavailable",
        &backend,
        &output.path.join("unavailable"),
        &[("g2_unavailable_helper", unavailable.as_path())],
    );
    let unavailable_stderr = String::from_utf8_lossy(&unavailable_result.stderr);
    assert!(!unavailable_result.status.success());
    assert!(unavailable_stderr.contains("MIR is unavailable for a device-reachable item"));
    assert!(unavailable_stderr.contains("g2_unavailable::fe2o3_kernel_unavailable"));
    assert!(unavailable_stderr.contains("g2_unavailable::local_bridge"));
    assert!(unavailable_stderr.contains("g2_unavailable_helper::unavailable"));
    assert!(unavailable_stderr.contains("reachable call chain:"));

    let malformed = compile_with_backend(
        &fixtures.join("malformed-registration.rs"),
        "g2_malformed",
        &backend,
        &output.path.join("malformed"),
        &[],
    );
    let malformed_stderr = String::from_utf8_lossy(&malformed.stderr);
    assert!(!malformed.status.success());
    assert!(malformed_stderr.contains("does not match registration magic"));
    assert!(!malformed_stderr.contains("[collector] root kernel:"));
}
