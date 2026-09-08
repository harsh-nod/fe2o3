use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch {
    source: PathBuf,
    target: PathBuf,
}

impl Scratch {
    fn new(case: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let scratch_root = std::env::var_os("FE2O3_GFX950_LOW_PRECISION_TEST_SCRATCH")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let name = format!(
            "fe2o3-gfx950-low-precision-ui-{case}-{}-{nonce}",
            std::process::id()
        );
        let source = scratch_root.join(&name);
        let target = std::env::var_os("FE2O3_GFX950_LOW_PRECISION_UI_TARGET_DIR")
            .map(PathBuf::from)
            .map_or_else(|| source.join("target"), |root| root.join(name));
        std::fs::create_dir_all(source.join("src")).expect("create UI fixture directory");
        Self { source, target }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.source);
        let _ = std::fs::remove_dir_all(&self.target);
    }
}

fn compile(case: &str, source: &str) -> Output {
    let scratch = Scratch::new(case);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repository root");
    std::fs::write(
        scratch.source.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-gfx950-low-precision-ui-{case}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\nfe2o3-host = {{ path = {:?} }}\n",
            root.join("crates/fe2o3-device"),
            root.join("crates/fe2o3-host"),
        ),
    )
    .expect("write UI fixture manifest");
    std::fs::write(scratch.source.join("src/lib.rs"), source).expect("write UI fixture source");
    Command::new(env!("CARGO"))
        .current_dir(&scratch.source)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env("CARGO_TARGET_DIR", &scratch.target)
        .env("FE2O3_CRATE_BINDING_ID_V1", "95".repeat(32))
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["check", "--offline", "--quiet"])
        .output()
        .expect("compile low-precision capability fixture")
}

#[test]
fn compiler_issued_low_precision_capabilities_typecheck() {
    let output = compile(
        "pass",
        include_str!("capability-ui/pass/low_precision_capabilities.rs"),
    );
    assert!(
        output.status.success(),
        "capability fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hostile_capability_substitutions_fail_closed() {
    const CASES: [(&str, &str, &[&str]); 12] = [
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
            "access-role-mismatch",
            include_str!("capability-ui/fail/access_role_mismatch.rs"),
            &["no method named `load`", "DisjointWrite"],
        ),
        (
            "alias-role-mismatch",
            include_str!("capability-ui/fail/alias_role_mismatch.rs"),
            &["mismatched types", "ExclusiveReadWrite", "DisjointWrite"],
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
            "matrix-operand-mismatch",
            include_str!("capability-ui/fail/matrix_operand_mismatch.rs"),
            &[
                "arguments to this method are incorrect",
                "Gfx950MfmaOperandA",
            ],
        ),
        (
            "matrix-profile-mismatch",
            include_str!("capability-ui/fail/matrix_profile_mismatch.rs"),
            &["mismatched types", "Gfx950Fp8E4M3", "Gfx950Fp4E2M1"],
        ),
        (
            "numerical-policy-mismatch",
            include_str!("capability-ui/fail/numerical_policy_mismatch.rs"),
            &["MatrixGlobalAccess", "PolicyBrand", "MatrixBrand"],
        ),
        (
            "tail-bounds-bypass",
            include_str!("capability-ui/fail/tail_bounds_bypass.rs"),
            &["no method named `load_unchecked`"],
        ),
        (
            "stale-epoch",
            include_str!("capability-ui/fail/stale_epoch.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "unsupported-target-capability",
            include_str!("capability-ui/fail/unsupported_target_capability.rs"),
            &["MatrixSubgroupWidth", "SubgroupWidth32"],
        ),
        (
            "logical-context-position",
            include_str!("capability-ui/fail/logical_context_position.rs"),
            &["logical KernelContext must be the first kernel parameter"],
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
fn shared_low_precision_capability_bridges_typecheck() {
    const BRIDGES: [(&str, &str); 5] = [
        (
            "global-matrix-view",
            include_str!("capability-ui/boundary/global_matrix_view.rs"),
        ),
        (
            "blocked-global-store",
            include_str!("capability-ui/boundary/blocked_global_store.rs"),
        ),
        (
            "policy-bound-gfx950",
            include_str!("capability-ui/boundary/policy_bound_gfx950.rs"),
        ),
        (
            "epoch-aware-transpose",
            include_str!("capability-ui/boundary/epoch_aware_transpose.rs"),
        ),
        (
            "branded-wave16",
            include_str!("capability-ui/boundary/branded_wave16.rs"),
        ),
    ];

    for (case, source) in BRIDGES {
        let output = compile(case, source);
        assert!(
            output.status.success(),
            "capability bridge {case} failed:\n{}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn transpose_to_next_epoch_typechecks() {
    let output = compile(
        "transpose-mfma-lifetime",
        include_str!("capability-ui/boundary/transpose_mfma_lifetime.rs"),
    );
    assert!(
        output.status.success(),
        "transpose handoff failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}
