use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn ui_target_dir(source: &Path, explicit: Option<&Path>, outer: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| outer.map(|root| root.join("tutorial-ui/fe2o3-gfx950-low-precision")))
        .unwrap_or_else(|| source.join("target"))
}

struct Scratch {
    source: PathBuf,
    target: PathBuf,
}

impl Scratch {
    fn new(case: &str) -> Self {
        let scratch_root = std::env::var_os("FE2O3_GFX950_LOW_PRECISION_TEST_SCRATCH")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let target =
            std::env::var_os("FE2O3_GFX950_LOW_PRECISION_UI_TARGET_DIR").map(PathBuf::from);
        // Resolve before nested Cargo changes directory; never reuse the outer target itself.
        let outer = std::env::var_os("CARGO_TARGET_DIR")
            .map(|root| std::path::absolute(root).expect("resolve outer Cargo target directory"));
        Self::new_in(case, &scratch_root, target.as_deref(), outer.as_deref())
    }

    fn new_in(
        case: &str,
        scratch_root: &Path,
        target: Option<&Path>,
        outer: Option<&Path>,
    ) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let name = format!(
            "fe2o3-gfx950-low-precision-ui-{case}-{}-{nonce}",
            std::process::id()
        );
        let source = scratch_root.join(&name);
        let target = ui_target_dir(&source, target, outer);
        std::fs::create_dir_all(source.join("src")).expect("create UI fixture directory");
        Self { source, target }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Only the default target is inside source; shared caches are caller-owned.
        let _ = std::fs::remove_dir_all(&self.source);
    }
}

#[test]
fn ui_target_selection_prefers_explicit_cache() {
    let explicit = Path::new("/caller/ui-cache");
    let source = Path::new("/scratch/case");
    for outer in [None, Some(Path::new("/caller/cargo-target"))] {
        assert_eq!(ui_target_dir(source, Some(explicit), outer), explicit);
    }
}

#[test]
fn ui_target_selection_nests_under_outer_cargo_target() {
    let outer = Path::new("/caller/cargo-target");
    let expected = outer.join("tutorial-ui/fe2o3-gfx950-low-precision");
    for source in [Path::new("/scratch/first"), Path::new("/scratch/second")] {
        let target = ui_target_dir(source, None, Some(outer));
        assert_eq!(target, expected);
        assert_ne!(target, outer);
        assert_ne!(target, source.join("target"));
    }
}

#[test]
fn ui_target_selection_defaults_to_owned_scratch_target() {
    let first = Path::new("/scratch/first");
    let second = Path::new("/scratch/second");
    assert_eq!(ui_target_dir(first, None, None), first.join("target"));
    assert_eq!(ui_target_dir(second, None, None), second.join("target"));
    assert_ne!(
        ui_target_dir(first, None, None),
        ui_target_dir(second, None, None)
    );
}

#[test]
fn scratch_cleanup_retains_shared_cache_and_removes_owned_targets() {
    let owner = Scratch::new_in("cache-cleanup", &std::env::temp_dir(), None, None);
    let owner_source = owner.source.clone();
    let cache = owner.target.clone();
    assert_eq!(cache, owner_source.join("target"));
    std::fs::create_dir_all(&cache).expect("create caller-owned test cache");
    let sentinel = cache.join("sentinel");
    std::fs::write(&sentinel, b"retained cache").expect("write shared cache sentinel");

    let first = Scratch::new_in("cache-first", &owner_source, Some(&cache), None);
    let second = Scratch::new_in("cache-second", &owner_source, Some(&cache), None);
    let first_source = first.source.clone();
    let second_source = second.source.clone();
    assert_ne!(first_source, second_source);
    assert_eq!(first.target, cache);
    assert_eq!(second.target, cache);
    assert!(first_source.join("src").is_dir());
    assert!(second_source.join("src").is_dir());

    drop(first);
    assert!(!first_source.exists());
    assert!(second_source.join("src").is_dir());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"retained cache");

    drop(second);
    assert!(!second_source.exists());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"retained cache");

    drop(owner);
    assert!(!owner_source.exists());
    assert!(!cache.exists());
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
            &[
                "error[E0277]",
                "MatrixGlobalAccess",
                "PolicyBrand",
                "MatrixBrand",
                "expected `MatrixBrand`, found `PolicyBrand`",
                "with_numerical_policy",
            ],
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
            &["KernelContext must be the first kernel parameter and may appear only once"],
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
    const BRIDGES: [(&str, &str); 6] = [
        (
            "same-kernel-numerical-policy",
            include_str!("capability-ui/boundary/same_kernel_numerical_policy.rs"),
        ),
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
