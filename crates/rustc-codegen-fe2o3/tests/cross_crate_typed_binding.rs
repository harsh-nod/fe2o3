use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TestOutputDir {
    path: PathBuf,
}

impl TestOutputDir {
    fn new(target_root: &Path) -> Self {
        let path = target_root.join(format!(
            "rustc-codegen-fe2o3-test-output/cross-crate-binding-{}",
            std::process::id()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale link output directory");
        }
        std::fs::create_dir_all(&path).expect("create link output directory");
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

fn cargo_target_root(workspace: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace.join(path),
        None => workspace.join("target"),
    }
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn same_logical_name_in_two_rlibs_resolves_distinct_artifacts() {
    let workspace = workspace();
    let cargo_target = cargo_target_root(&workspace);
    let fixture_root =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/cross-crate-binding");
    let cargo_fe2o3 = cargo_target.join("debug/cargo-fe2o3");
    let backend = cargo_target.join("debug/librustc_codegen_fe2o3.so");

    let backend_build = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "build",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "-p",
            "cargo-fe2o3",
            "--features",
            "rustc-codegen-fe2o3/qualification-oracles-test-only,cargo-fe2o3/qualification-oracles-test-only",
        ])
        .output()
        .expect("build qualification compiler tools");
    require_success("qualification compiler tools build", &backend_build);
    assert!(
        backend.is_file(),
        "missing backend at {}",
        backend.display()
    );
    assert!(
        cargo_fe2o3.is_file(),
        "missing cargo-fe2o3 at {}",
        cargo_fe2o3.display()
    );

    let output_dir = TestOutputDir::new(&cargo_target);
    let fixture_target = output_dir.path.join("cargo-target");
    let kernel_a = build_kernel(
        &cargo_fe2o3,
        &backend,
        &fixture_root.join("kernel-a"),
        &fixture_target,
        "a",
    );
    let kernel_b = build_kernel(
        &cargo_fe2o3,
        &backend,
        &fixture_root.join("kernel-b"),
        &fixture_target,
        "b",
    );

    let executable = output_dir.path.join("binding-link-app");
    let source = fixture_root.join("app/src/main.rs");
    let rocm_path = std::env::var_os("ROCM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/rocm"));
    let rocm_library = rocm_path.join("lib");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let link = Command::new(rustc)
        .current_dir(&fixture_root)
        .arg(&source)
        .args(["--edition=2024", "-o"])
        .arg(&executable)
        .arg("--extern")
        .arg(format!("kernel_a={}", kernel_a.display()))
        .arg("--extern")
        .arg(format!("kernel_b={}", kernel_b.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            kernel_a.parent().expect("kernel A deps").display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            kernel_b.parent().expect("kernel B deps").display()
        ))
        .arg("-L")
        .arg(format!("native={}", rocm_library.display()))
        .arg("-C")
        .arg(format!("link-arg=-Wl,-rpath,{}", rocm_library.display()))
        .output()
        .expect("link fixture executable");
    require_success("fixture link", &link);

    let run = Command::new(&executable)
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", executable.display()));
    require_success("fixture executable", &run);
}

fn build_kernel(
    cargo_fe2o3: &Path,
    backend: &Path,
    package_root: &Path,
    target_dir: &Path,
    label: &str,
) -> PathBuf {
    let manifest = package_root.join("Cargo.toml");
    let mut command = Command::new(cargo_fe2o3);
    command
        .current_dir(package_root)
        .args(["build", "--locked", "--manifest-path"])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .env("FE2O3_BACKEND", backend)
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .env(
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "1",
        )
        .env(
            "FE2O3_TARGET",
            std::env::var("FE2O3_TEST_TARGET").unwrap_or_else(|_| "gfx942:xnack-".to_owned()),
        );
    for variable in [
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_TARGET_DIR",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
    ] {
        command.env_remove(variable);
    }
    scrub_dynamic_loader_environment(&mut command);
    let build = command.output().expect("build kernel fixture");
    require_success(&format!("kernel {label} build"), &build);

    let prefix = format!("libfe2o3_binding_kernel_{label}-");
    let deps = target_dir.join("debug/deps");
    let mut matches = std::fs::read_dir(&deps)
        .expect("read fixture dependencies")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("rlib")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix))
        })
        .collect::<Vec<_>>();
    matches.sort();
    assert_eq!(
        matches.len(),
        1,
        "expected one {prefix}*.rlib in {}",
        deps.display()
    );
    matches.pop().unwrap()
}

fn scrub_dynamic_loader_environment(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        let name_bytes = name.as_bytes();
        if name_bytes.starts_with(b"LD_")
            || name_bytes.starts_with(b"DYLD_")
            || name_bytes == b"GLIBC_TUNABLES"
        {
            command.env_remove(name);
        }
    }
}

fn require_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
