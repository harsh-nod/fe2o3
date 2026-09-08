use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-wave64-capability-compile-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).expect("create compile fixture");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn attributed_wave64_source_typechecks_for_the_device_target() {
    let scratch = Scratch::new();
    let target = std::env::var_os("FE2O3_WAVE64_COMPILE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    let device = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/fe2o3-device")
        .canonicalize()
        .expect("canonical fe2o3-device path");
    std::fs::create_dir_all(scratch.0.join("host-stub/src")).expect("create host stub");
    std::fs::write(
        scratch.0.join("host-stub/Cargo.toml"),
        "[package]\nname = \"fe2o3-host\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\npath = \"src/lib.rs\"\n",
    )
    .expect("write host-stub manifest");
    std::fs::write(scratch.0.join("host-stub/src/lib.rs"), "").expect("write empty host stub");
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-wave64-capability-compile\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\n\n[target.'cfg(not(target_arch = \"amdgpu\"))'.dependencies]\nfe2o3-host = {{ path = \"host-stub\" }}\n",
            device
        ),
    )
    .expect("write compile manifest");
    std::fs::write(
        scratch.0.join("src/lib.rs"),
        format!("#![no_std]\n{}", include_str!("../src/kernel.rs")),
    )
    .expect("write exact wave64 source");
    let output = Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args([
            "check",
            "--offline",
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(target)
        .output()
        .expect("check exact Wave64 device source");
    assert!(
        output.status.success(),
        "Wave64 capability source did not typecheck:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn revised_source_shape_still_matches_the_cpu_correspondence() {
    let scratch = Scratch::new();
    let target = std::env::var_os("FE2O3_WAVE64_CORRESPONDENCE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        "[package]\nname = \"fe2o3-wave64-correspondence-check\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nquote = \"1\"\nsha2 = { version = \"0.11\", default-features = false }\nsyn = { version = \"2\", features = [\"full\"] }\n",
    )
    .expect("write correspondence manifest");
    for (name, source) in [
        ("contract.rs", include_str!("../src/contract.rs")),
        ("kernel.rs", include_str!("../src/kernel.rs")),
        ("oracle.rs", include_str!("../src/oracle.rs")),
        (
            "source_model_correspondence.rs",
            include_str!("../src/source_model_correspondence.rs"),
        ),
    ] {
        std::fs::write(scratch.0.join("src").join(name), source)
            .unwrap_or_else(|error| panic!("write {name}: {error}"));
    }
    std::fs::write(
        scratch.0.join("src/lib.rs"),
        r#"
pub mod contract;
pub mod oracle;
pub mod source_model_correspondence;

pub use contract::{MAX_EXACT_INPUT_MAGNITUDE_V1, WAVE64_LANES_V1, lane_is_active_v1};
pub use oracle::{
    CollectiveOutputV1, OracleErrorV1, wave64_collectives_oracle_v1,
};

#[test]
fn exact_source_shape_is_admitted() {
    assert!(source_model_correspondence::collect_reviewed_source_algorithm_v2(
        include_str!("kernel.rs")
    ).is_ok());
}
"#,
    )
    .expect("write correspondence harness");
    let output = Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env("CARGO_TARGET_DIR", target)
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["test", "--offline", "--lib", "--quiet"])
        .output()
        .expect("check source-to-CPU correspondence");
    assert!(
        output.status.success(),
        "source-to-CPU correspondence failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
