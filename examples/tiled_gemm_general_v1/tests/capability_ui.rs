use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn native_ui_target_dir(source: &Path, explicit: Option<&Path>, outer: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| outer.map(|root| root.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/native")))
        .unwrap_or_else(|| source.join("target"))
}

#[test]
fn native_ui_target_selection_prefers_explicit_cache() {
    let source = Path::new("/scratch/case");
    for explicit in [
        Path::new("/caller/ui-cache"),
        Path::new("relative-ui-cache"),
    ] {
        for outer in [None, Some(Path::new("/caller/cargo-target"))] {
            assert_eq!(
                native_ui_target_dir(source, Some(explicit), outer),
                explicit
            );
        }
    }
}

#[test]
fn native_ui_target_selection_nests_under_outer_cargo_target() {
    let outer = Path::new("/caller/cargo-target");
    let expected = outer.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/native");
    for source in [Path::new("/scratch/first"), Path::new("/scratch/second")] {
        let target = native_ui_target_dir(source, None, Some(outer));
        assert_eq!(target, expected);
        assert_ne!(target, outer);
        assert!(!target.starts_with(source));
        assert_ne!(
            target,
            outer.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/amdgpu")
        );
    }
}

#[test]
fn native_ui_target_selection_defaults_to_owned_scratch_target() {
    let first = Path::new("/scratch/first");
    let second = Path::new("/scratch/second");
    assert_eq!(
        native_ui_target_dir(first, None, None),
        first.join("target")
    );
    assert_eq!(
        native_ui_target_dir(second, None, None),
        second.join("target")
    );
    assert_ne!(
        native_ui_target_dir(first, None, None),
        native_ui_target_dir(second, None, None)
    );
}

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
    let explicit = std::env::var_os("FE2O3_GENERAL_GEMM_UI_TARGET_DIR").map(PathBuf::from);
    // Resolve before nested Cargo changes directory; never reuse the outer target itself.
    let outer = std::env::var_os("CARGO_TARGET_DIR")
        .map(|root| std::path::absolute(root).expect("resolve outer Cargo target directory"));
    let target = native_ui_target_dir(&scratch.0, explicit.as_deref(), outer.as_deref());
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

#[path = "capability-ui/amdgpu.rs"]
mod amdgpu;

#[test]
fn supported_typed_gemm_bridges_typecheck_on_amdgpu() {
    for (case, source) in [
        (
            "typed-global-epilogue",
            include_str!("capability-ui/pass/typed_global_epilogue.rs"),
        ),
        (
            "same-policy-binding",
            include_str!("capability-ui/pass/same_policy_binding.rs"),
        ),
    ] {
        amdgpu::compile(case, source).assert_success(case);
    }
}

#[test]
fn legacy_substitutions_and_policy_custody_fail_closed_on_amdgpu() {
    // A failed dependency or target setup must not masquerade as a negative.
    amdgpu::compile(
        "negative-device-control",
        include_str!("capability-ui/pass/same_policy_binding.rs"),
    )
    .assert_success("negative-device-control");

    const CASES: [(&str, &str, &str, &str, &[&str]); 5] = [
        (
            "legacy-global-matrix",
            include_str!("capability-ui/boundary/global_matrix_bridge.rs"),
            "E0308",
            "matrix.bf16_a_row_major(input, 0, 16, 16, 16)",
            &["mismatched types", "&[u16]"],
        ),
        (
            "legacy-epoch-reassignment",
            include_str!("capability-ui/boundary/dynamic_workgroup_epoch_loop.rs"),
            "E0308",
            "workgroup = next;",
            &["mismatched types", "InitialEpoch", "NextEpoch"],
        ),
        (
            "missing-policy-owner",
            include_str!("capability-ui/boundary/numerical_policy_binding.rs"),
            "E0061",
            "matrix.with_numerical_policy::<Brand<'kernel>, StrictIeee>()",
            &["1 argument", "0 arguments", "NumericalPolicyCapability"],
        ),
        (
            "cross-kernel-policy",
            include_str!("capability-ui/fail/cross_policy.rs"),
            "E0308",
            "matrix.with_numerical_policy(policy)",
            &[
                "mismatched types",
                "NumericalPolicyCapability",
                "KernelA",
                "KernelB",
            ],
        ),
        (
            "cross-kernel-matrix-lane",
            include_str!("capability-ui/fail/cross_brand.rs"),
            "E0308",
            "matrix.bf16_zero_accumulator(lane)",
            &["mismatched types", "SubgroupLane", "KernelA", "KernelB"],
        ),
    ];
    for (case, source, code, source_line, expected) in CASES {
        amdgpu::compile(case, source).assert_rejected(case, code, source_line, expected);
    }
}
