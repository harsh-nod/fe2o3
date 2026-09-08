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
            "fe2o3-general-gemm-ui-{case}-{}-{nonce}",
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
    std::fs::create_dir_all(scratch.0.join("host-stub/src")).expect("create host stub");
    std::fs::write(
        scratch.0.join("host-stub/Cargo.toml"),
        "[package]\nname = \"fe2o3-host\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\npath = \"src/lib.rs\"\n",
    )
    .expect("write host-stub manifest");
    std::fs::write(scratch.0.join("host-stub/src/lib.rs"), "").expect("write host stub source");
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-general-gemm-ui-{case}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\nfe2o3-host = {{ path = \"host-stub\" }}\n",
            device
        ),
    )
    .expect("write UI fixture manifest");
    std::fs::write(scratch.0.join("src/lib.rs"), source).expect("write UI fixture source");
    let target = std::env::var_os("FE2O3_GENERAL_GEMM_UI_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    let action = if case == "pipeline-resource-limit" {
        "build"
    } else {
        "check"
    };
    Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env("CARGO_TARGET_DIR", target)
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args([action, "--offline", "--quiet"])
        .output()
        .expect("compile GEMM capability UI fixture")
}

#[test]
fn supported_capability_shape_typechecks() {
    let output = compile(
        "pass",
        include_str!("capability-ui/pass/capability_shape.rs"),
    );
    assert!(
        output.status.success(),
        "supported capability shape failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hostile_capability_substitutions_fail_closed() {
    const CASES: [(&str, &str, &[&str]); 10] = [
        (
            "wrong-layout",
            include_str!("capability-ui/fail/wrong_layout.rs"),
            &["arguments to this method are incorrect", "MfmaOperandA"],
        ),
        (
            "wrong-brand",
            include_str!("capability-ui/fail/wrong_brand.rs"),
            &["mismatched types", "Left", "Right"],
        ),
        (
            "wrong-epoch",
            include_str!("capability-ui/fail/wrong_epoch.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "unsupported-matrix-width",
            include_str!("capability-ui/fail/unsupported_matrix_width.rs"),
            &["MatrixSubgroupWidth", "SubgroupWidth32"],
        ),
        (
            "unsupported-matrix-type",
            include_str!("capability-ui/fail/unsupported_matrix_type.rs"),
            &["mismatched types", "u16", "f32"],
        ),
        (
            "invalid-extents-strides",
            include_str!("capability-ui/fail/invalid_extents_strides.rs"),
            &["no method named `bf16_a_row_major_unchecked`"],
        ),
        (
            "missing-barrier",
            include_str!("capability-ui/fail/missing_barrier.rs"),
            &["no method named `read`"],
        ),
        (
            "reordered-barrier",
            include_str!("capability-ui/fail/reordered_barrier.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "target-capability-mismatch",
            include_str!("capability-ui/fail/target_capability_mismatch.rs"),
            &["mismatched types", "SubgroupWidth64", "SubgroupWidth32"],
        ),
        (
            "pipeline-resource-limit",
            include_str!("capability-ui/fail/pipeline_resource_limit.rs"),
            &[
                "evaluation panicked",
                "pipeline buffer count exceeds the compiler limit",
            ],
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

#[test]
fn missing_fully_typed_gemm_bridges_remain_explicit() {
    const BOUNDARIES: [(&str, &str, &[&str]); 4] = [
        (
            "global-matrix-bridge",
            include_str!("capability-ui/boundary/global_matrix_bridge.rs"),
            &["mismatched types", "&[u16]"],
        ),
        (
            "dynamic-workgroup-epoch-loop",
            include_str!("capability-ui/boundary/dynamic_workgroup_epoch_loop.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "typed-global-epilogue",
            include_str!("capability-ui/boundary/typed_global_epilogue.rs"),
            &[
                "Global's role must be ReadOnly, DisjointWrite<IndexSpace>, or AtomicReadWrite<Scope>",
            ],
        ),
        (
            "numerical-policy-binding",
            include_str!("capability-ui/boundary/numerical_policy_binding.rs"),
            &["no method named `with_numerical_policy`"],
        ),
    ];

    for (case, source, expected) in BOUNDARIES {
        assert!(source.contains("expected-boundary: FE2O3-CAP-GEMM"));
        let output = compile(case, source);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(!output.status.success(), "boundary fixture {case} compiled");
        for expected in expected {
            assert!(
                stderr.contains(expected),
                "{case} no longer fails at {expected:?}:\n{stderr}"
            );
        }
    }
}
