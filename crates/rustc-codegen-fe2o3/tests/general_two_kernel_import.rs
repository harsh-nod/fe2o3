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
