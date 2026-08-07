use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OUTPUT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const FIXTURE: &str = r#"
#![allow(dead_code)]

#[inline(never)]
fn shared(value: u32) -> u32 {
    value
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_second(value: u32) -> u32 {
    shared(value)
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_first(value: u32) -> u32 {
    shared(value)
}

#[used]
static __fe2o3_kernel_registration_second: (
    u64, u16, u16, &'static str, &'static str, fn(u32) -> u32,
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "second", "second", fe2o3_kernel_second,
);

#[used]
static __fe2o3_kernel_registration_first: (
    u64, u16, u16, &'static str, &'static str, fn(u32) -> u32,
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "first", "first", fe2o3_kernel_first,
);
"#;

struct TestOutputDir {
    path: PathBuf,
}

impl TestOutputDir {
    fn new() -> Self {
        let workspace = workspace();
        let path = workspace.join(format!(
            "target/rustc-codegen-fe2o3-test-output/general-two-kernel-{}-{}",
            std::process::id(),
            NEXT_OUTPUT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale general two-kernel output");
        }
        std::fs::create_dir_all(&path).expect("create general two-kernel output");
        std::fs::write(path.join("fixture.rs"), FIXTURE).expect("write general two-kernel fixture");
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

#[test]
fn general_two_kernel_fixture_clears_the_standard_rust_frontend() {
    let output = TestOutputDir::new();
    let compiled = rustc()
        .arg(output.path.join("fixture.rs"))
        .args(["--edition=2024", "--crate-type=lib", "--emit=metadata"])
        .arg("-o")
        .arg(output.path.join("fixture.rmeta"))
        .output()
        .expect("compile general two-kernel fixture");
    require_success("general two-kernel frontend", &compiled);
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn general_two_kernel_backend_dump_is_stable_and_shares_one_helper_identity() {
    let workspace = workspace();
    let output = TestOutputDir::new();
    let built = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build codegen backend");
    require_success("codegen backend build", &built);
    let backend = workspace.join("target/debug/librustc_codegen_fe2o3.so");

    let first = compile_with_backend(&output, &backend, "first");
    let second = compile_with_backend(&output, &backend, "second");
    let first_dump = mir_dump(&String::from_utf8_lossy(&first.stderr));
    let second_dump = mir_dump(&String::from_utf8_lossy(&second.stderr));
    assert!(
        !first_dump.is_empty(),
        "first compiler run did not dump MIR"
    );
    assert_eq!(first_dump, second_dump);

    let function_rows = first_dump
        .lines()
        .filter(|line| line.trim_start().starts_with('['))
        .collect::<Vec<_>>();
    assert_eq!(function_rows.len(), 3, "unexpected MIR dump:\n{first_dump}");
    assert!(function_rows[0].contains("kernel-entry"));
    assert!(function_rows[1].contains("kernel-entry"));
    assert!(function_rows[2].contains("internal-helper"));
    assert_eq!(first_dump.matches("source identity v1:").count(), 3);
    assert_eq!(
        first_dump
            .matches("path: general_two_kernel::shared")
            .count(),
        1
    );
}

fn compile_with_backend(output: &TestOutputDir, backend: &Path, label: &str) -> Output {
    let artifacts = output.path.join(format!("{label}-artifacts"));
    std::fs::create_dir_all(&artifacts).expect("create artifact directory");
    rustc()
        .arg(output.path.join("fixture.rs"))
        .args(["--edition=2024", "--crate-type=lib", "--crate-name"])
        .arg("general_two_kernel")
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg("-o")
        .arg(output.path.join(format!("lib{label}.rlib")))
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_CODEGEN_PIPELINE", "legacy-v1")
        .env("FE2O3_DUMP_MIR", "1")
        .env("FE2O3_HSACO_DIR", artifacts)
        .output()
        .expect("compile fixture with fe2o3 backend")
}

fn mir_dump(stderr: &str) -> String {
    let Some(start) = stderr.find("=== fe2o3 MIR import scaffold") else {
        return String::new();
    };
    let remainder = &stderr[start..];
    let Some(end) = remainder.find("===================================") else {
        return String::new();
    };
    remainder[..end + "===================================".len()].to_owned()
}
