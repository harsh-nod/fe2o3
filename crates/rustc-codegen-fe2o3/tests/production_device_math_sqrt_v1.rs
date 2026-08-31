#![deny(warnings)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct ScratchTarget {
    path: PathBuf,
}

impl ScratchTarget {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-device-math-{label}-{}-{nonce}",
            std::process::id(),
        ));
        std::fs::create_dir(&path).expect("create device-math extraction target directory");
        Self { path }
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

fn materialize_f64_fixture(scratch: &ScratchTarget) -> PathBuf {
    let fixture = scratch.path.join("f64-fixture");
    std::fs::create_dir_all(fixture.join("src")).expect("create f64 fixture source directory");
    let root = workspace();
    let manifest = format!(
        r#"[package]
name = "fe2o3-production-ranked-bounds-fixture"
version = "0.1.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
fe2o3-device = {{ path = "{}" }}

[target.'cfg(not(target_arch = "amdgpu"))'.dependencies]
fe2o3-host = {{ path = "{}" }}

[lib]
name = "fe2o3_production_ranked_bounds_fixture"
path = "src/lib.rs"
"#,
        root.join("crates/fe2o3-device").display(),
        root.join("crates/fe2o3-host").display(),
    );
    std::fs::write(fixture.join("Cargo.toml"), manifest).expect("write f64 fixture manifest");
    std::fs::copy(root.join("Cargo.lock"), fixture.join("Cargo.lock"))
        .expect("copy pinned workspace lockfile into f64 fixture");
    std::fs::write(
        fixture.join("src/lib.rs"),
        include_str!("fixtures/production-device-math-sqrt-f64.rs"),
    )
    .expect("write f64 fixture source");
    fixture
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn compiler_authenticated_sqrt_reaches_exact_gfx942_and_gfx950_llvm() {
    for (cpu, target, expected, forbidden) in [
        (
            "gfx942",
            "gfx942:xnack-",
            "call float @llvm.experimental.constrained.sqrt.f32(float",
            "call float @llvm.sqrt.f32(float",
        ),
        (
            "gfx950",
            "gfx950:xnack-",
            "call float @llvm.sqrt.f32(float",
            "llvm.experimental.constrained.sqrt.f32",
        ),
    ] {
        let scratch = ScratchTarget::new(cpu);
        let llvm_path = scratch.path.join("device-math-sqrt.ll");
        let output = run_extraction(&scratch, cpu, "device_math_sqrt", Some(&llvm_path));
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
        assert!(
            output.status.success(),
            "typed DeviceMath::sqrt_f32 did not lower for {target}:\n{stderr}"
        );
        assert!(
            stderr.contains("Rust -> semantic MIR -> ranked PLIRON -> Kernel IR")
                && stderr.contains(&format!("composed formal/ranked memory -> {target} LLVM"))
                && stderr.contains("artifact/launch authority false"),
            "production sqrt extraction omitted its target-bound receipt for {target}:\n{stderr}",
        );

        let llvm = std::fs::read_to_string(&llvm_path)
            .unwrap_or_else(|error| panic!("read {target} sqrt LLVM: {error}"));
        for required in [
            "target triple = \"amdgcn-amd-amdhsa\"",
            &format!("\"target-cpu\"=\"{cpu}\""),
            "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
            "\"unsafe-fp-math\"=\"false\"",
            "\"approx-func-fp-math\"=\"false\"",
            "\"fp-contract\"=\"off\"",
            "@device_math_sqrt",
            expected,
        ] {
            assert!(
                llvm.contains(required),
                "production {target} sqrt LLVM omitted {required:?}:\n{llvm}",
            );
        }
        assert!(
            !llvm.contains(forbidden),
            "production {target} sqrt LLVM used forbidden intrinsic {forbidden:?}:\n{llvm}",
        );
        assert!(
            !llvm.contains("call fast float")
                && (cpu != "gfx942"
                    || llvm
                        .contains("metadata !\"round.tonearest\", metadata !\"fpexcept.ignore\"",)),
            "production {target} sqrt LLVM changed the authenticated IEEE policy:\n{llvm}",
        );
        assert_eq!(
            llvm.matches(expected).count(),
            1,
            "production {target} LLVM must contain exactly one authenticated sqrt call:\n{llvm}",
        );
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn sqrt_rejects_unsupported_scalar_type_before_llvm_publication() {
    let scratch = ScratchTarget::new("f64");
    let llvm_path = scratch.path.join("must-not-exist.ll");
    let fixture = materialize_f64_fixture(&scratch);
    let output = run_f64_extraction(&scratch, &fixture, &llvm_path);
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        !output.status.success()
            && stderr.contains("expected `f32`, found `f64`")
            && !llvm_path.exists(),
        "unsupported f64 sqrt did not fail before LLVM publication:\n{stderr}",
    );
}

fn run_f64_extraction(scratch: &ScratchTarget, fixture: &Path, llvm_path: &Path) -> Output {
    let mut command = production_command(
        fixture,
        "fe2o3_production_ranked_bounds_fixture",
        "gfx942",
        &scratch.path.join("cargo"),
        false,
    );
    command
        .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", llvm_path)
        .arg("--lib");
    command
        .output()
        .expect("run production f64 sqrt extraction")
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn sqrt_rejects_unsupported_amd_target_before_llvm_publication() {
    let scratch = ScratchTarget::new("gfx90a");
    let llvm_path = scratch.path.join("must-not-exist.ll");
    let output = run_extraction(&scratch, "gfx90a", "device_math_sqrt", Some(&llvm_path));
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        !output.status.success()
            && stderr.contains(
                "production compilation requires live rustc target CPU \"gfx942\" or \"gfx950\"; found \"gfx90a\"",
            )
            && !llvm_path.exists(),
        "unsupported target sqrt did not fail before LLVM publication:\n{stderr}",
    );
}

fn run_extraction(
    scratch: &ScratchTarget,
    cpu: &str,
    feature: &str,
    llvm_path: Option<&Path>,
) -> Output {
    let root = workspace();
    let mut command = production_command(
        &root,
        "fe2o3_production_ranked_bounds_fixture",
        cpu,
        &scratch.path,
        true,
    );
    command.args([
        "-p",
        "fe2o3-production-ranked-bounds-fixture",
        "--features",
        feature,
    ]);
    if let Some(llvm_path) = llvm_path {
        command.env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", llvm_path);
    }
    command.output().expect("run production sqrt extraction")
}

fn production_command(
    current_dir: &Path,
    crate_name: &str,
    cpu: &str,
    target_dir: &Path,
    locked: bool,
) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(current_dir)
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env("FE2O3_EXTRACT_CRATE_V1", crate_name)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "55".repeat(32),
        )
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            format!(
                "-Zalways-encode-mir -Ctarget-cpu={cpu} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
            ),
        )
        .arg("check");
    command.arg(if locked { "--locked" } else { "--offline" });
    command
        .args([
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(target_dir);
    command
}
