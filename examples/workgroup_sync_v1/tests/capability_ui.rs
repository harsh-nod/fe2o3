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
            "fe2o3-workgroup-capability-ui-{case}-{}-{nonce}",
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

fn compile(case: &str, source: &str, needs_host_adapter: bool) -> Output {
    let scratch = Scratch::new(case);
    let device = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/fe2o3-device")
        .canonicalize()
        .expect("canonical fe2o3-device path");
    let host_dependency = needs_host_adapter.then(|| {
        let host = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/fe2o3-host")
            .canonicalize()
            .expect("canonical fe2o3-host path");
        format!("fe2o3-host = {{ path = {host:?} }}\n")
    });
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-capability-ui-{case}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {device:?} }}\n{}",
            host_dependency.as_deref().unwrap_or_default(),
        ),
    )
    .expect("write UI fixture manifest");
    std::fs::write(scratch.0.join("src/lib.rs"), source).expect("write UI fixture source");
    let mut command = Command::new(env!("CARGO"));
    let target = std::env::var_os("FE2O3_CAPABILITY_UI_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    command
        .current_dir(&scratch.0)
        .env("CARGO_TARGET_DIR", target)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["check", "--offline", "--quiet"]);
    command.output().expect("compile capability UI fixture")
}

#[test]
fn generic_epoch_aware_collective_shape_typechecks() {
    let output = compile(
        "generic-collective-shape",
        include_str!("../src/capability_collectives.rs"),
        false,
    );
    assert!(
        output.status.success(),
        "generic workgroup source shape did not typecheck:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn synchronization_typestate_rejects_hostile_source() {
    const CASES: [(&str, &str, &[&str]); 8] = [
        (
            "conditional-barrier",
            include_str!("fixtures/conditional_barrier.rs"),
            &["`if` and `else` have incompatible types", "NextEpoch"],
        ),
        (
            "missing-barrier",
            include_str!("fixtures/missing_barrier.rs"),
            &["no method named `load`", "WorkgroupAddressSpace"],
        ),
        (
            "reordered-barrier",
            include_str!("fixtures/reordered_barrier.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "stale-epoch",
            include_str!("fixtures/stale_epoch.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "output-collision",
            include_str!("fixtures/output_collision.rs"),
            &["mismatched types", "DisjointIndex"],
        ),
        (
            "wrong-subgroup-width",
            include_str!("fixtures/wrong_subgroup_width.rs"),
            &["mismatched types", "SubgroupWidth32"],
        ),
        (
            "wrong-workgroup-brand",
            include_str!("fixtures/wrong_workgroup_brand.rs"),
            &["mismatched types", "Right"],
        ),
        (
            "uninitialized-lds-read",
            include_str!("fixtures/uninitialized_lds_read.rs"),
            &["no method named `read`", "WorkgroupLdsUninitialized"],
        ),
    ];

    for (case, source, expected) in CASES {
        let output = compile(case, source, false);
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

#[test]
fn semantic_sync_negatives_reach_the_compiler_analysis_boundary() {
    const CASES: [(&str, &str, &str); 3] = [
        (
            "incomplete-collective-participation",
            include_str!("fixtures/incomplete_collective_participation.rs"),
            "FE2O3-CAP-ANALYSIS001",
        ),
        (
            "insufficient-atomic-scope",
            include_str!("fixtures/insufficient_atomic_scope.rs"),
            "FE2O3-CAP-ANALYSIS001",
        ),
        (
            "insufficient-atomic-ordering",
            include_str!("fixtures/insufficient_atomic_ordering.rs"),
            "FE2O3-CAP-ANALYSIS001",
        ),
    ];

    for (case, source, diagnostic) in CASES {
        assert!(source.contains(&format!("expected-boundary: {diagnostic}")));
        let output = compile(case, source, true);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(
            output.status.success(),
            "{case} was rejected before MIR/final-graph analysis:\n{stderr}"
        );
        assert!(
            !stderr.contains(diagnostic),
            "a direct Rust check cannot claim final-graph rejection"
        );
    }
}
