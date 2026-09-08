use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(case: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-advanced-attention-ui-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).expect("create UI fixture directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn compile(case: &str, source: &str) -> Output {
    let scratch = Scratch::new(case);
    let device = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/fe2o3-device")
        .canonicalize()
        .expect("canonical fe2o3-device path");
    let host = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/fe2o3-host")
        .canonicalize()
        .expect("canonical fe2o3-host path");
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-advanced-attention-ui-{case}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {device:?} }}\nfe2o3-host = {{ path = {host:?} }}\n",
        ),
    )
    .expect("write UI fixture manifest");
    std::fs::write(scratch.0.join("src/lib.rs"), source).expect("write UI fixture source");
    let target = std::env::var_os("FE2O3_ADVANCED_ATTENTION_UI_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env("CARGO_TARGET_DIR", target)
        .env("FE2O3_CRATE_BINDING_ID_V1", "a7".repeat(32))
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["check", "--offline", "--quiet"])
        .output()
        .expect("compile advanced-attention capability fixture")
}

#[test]
fn compiler_issued_attention_capabilities_typecheck() {
    let output = compile(
        "pass",
        include_str!("capability-ui/pass/attention_capabilities.rs"),
    );
    assert!(
        output.status.success(),
        "genuine capability fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn typed_fp8_transpose_path_typechecks() {
    let output = compile(
        "typed-fp8-transpose",
        include_str!("capability-ui/pass/typed_fp8_transpose.rs"),
    );
    assert!(
        output.status.success(),
        "typed FP8 transpose fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hostile_capability_substitutions_fail_closed() {
    const CASES: [(&str, &str, &[&str]); 7] = [
        (
            "context-forgery",
            include_str!("capability-ui/fail/context_forgery.rs"),
            &["KernelContextTypeV1"],
        ),
        (
            "cross-kernel-brand",
            include_str!("capability-ui/fail/cross_kernel_brand.rs"),
            &["mismatched types", "OutputBrand", "IndexBrand"],
        ),
        (
            "address-space-mismatch",
            include_str!("capability-ui/fail/address_space_mismatch.rs"),
            &[
                "mismatched types",
                "GlobalAddressSpace",
                "PrivateAddressSpace",
            ],
        ),
        (
            "access-role-mismatch",
            include_str!("capability-ui/fail/access_role_mismatch.rs"),
            &["no method named `load`", "DisjointWrite"],
        ),
        (
            "stale-lds-epoch",
            include_str!("capability-ui/fail/stale_lds_epoch.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "barrier-epoch-misuse",
            include_str!("capability-ui/fail/barrier_epoch_misuse.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "unsupported-target-capability",
            include_str!("capability-ui/fail/unsupported_target_capability.rs"),
            &["MatrixSubgroupWidth", "SubgroupWidth32"],
        ),
    ];

    for (case, source, expected) in CASES {
        let output = compile(case, source);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(!output.status.success(), "hostile fixture {case} compiled");
        for expected in expected {
            assert!(
                stderr.contains(expected),
                "{case} omitted {expected:?}:\n{stderr}"
            );
        }
    }
}
