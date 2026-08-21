use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct ScratchTarget {
    path: PathBuf,
}

impl ScratchTarget {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-extraction-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create extraction target directory");
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

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn attributed_kernel_is_recollected_inside_a_real_amdgcn_dependency_graph() {
    let target = ScratchTarget::new();
    let repeated_target = ScratchTarget::new();
    let first = run_extraction(&target);
    let repeated = run_extraction(&repeated_target);

    assert_eq!(
        identity_inventory_sha256(&first),
        identity_inventory_sha256(&repeated),
        "separate AMD rustc processes derived different identity inventories",
    );
    assert_eq!(
        preflight_plan_sha256(&first),
        preflight_plan_sha256(&repeated),
        "separate AMD rustc processes derived different raw-MIR preflight plans",
    );
    assert_eq!(
        semantic_mir_sha256(&first),
        semantic_mir_sha256(&repeated),
        "separate AMD rustc processes admitted different canonical semantic MIR requests",
    );
}

fn run_extraction(target: &ScratchTarget) -> String {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_extraction_fixture",
        )
        // The production cargo-fe2o3 parent owns this observation. This
        // process-isolation fixture has no production authority and supplies
        // only a nonzero test value so collection can reach the importer gate.
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-extraction-fixture",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(&target.path)
        .output()
        .expect("run AMD extraction fixture");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");

    assert!(
        !output.status.success(),
        "production pipeline unexpectedly passed the pending semantic middle-end boundary"
    );
    let inventory_sha256 = identity_inventory_sha256(&stderr);
    let preflight_sha256 = preflight_plan_sha256(&stderr);
    let semantic_sha256 = semantic_mir_sha256(&stderr);
    let expected_milestone = format!(
        "production-v1 semantic importer authenticated rustc identity inventory {inventory_sha256} and bounded preflight plan {preflight_sha256}, then admitted one complete semantic MIR request with 1 function(s), 3 callable(s), and canonical identity {semantic_sha256}; semantic middle-end construction remains pending; no fallback or artifact emission was entered",
    );
    assert!(
        stderr.contains(&expected_milestone),
        "missing exact admitted semantic MIR milestone diagnostic {expected_milestone:?}:\n{stderr}"
    );
    for forbidden in [
        "semantic import target rejection",
        "semantic importer rejected complete semantic MIR",
        "semantic importer rejected semantic body construction",
        "requires authoritative rustc LLVM target",
        "found no registered kernel",
        "body record construction remains pending",
        "schema-shaped semantic",
        "legacy-v1",
        "kernel-ir-v1",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "AMD extraction entered forbidden path {forbidden:?}:\n{stderr}"
        );
    }
    stderr
}

fn identity_inventory_sha256(stderr: &str) -> &str {
    canonical_sha256_after(
        stderr,
        "authenticated rustc identity inventory ",
        " and bounded preflight plan ",
        "rustc identity inventory",
    )
}

fn preflight_plan_sha256(stderr: &str) -> &str {
    canonical_sha256_after(
        stderr,
        "and bounded preflight plan ",
        ", then admitted one complete semantic MIR request with ",
        "rustc preflight plan",
    )
}

fn semantic_mir_sha256(stderr: &str) -> &str {
    canonical_sha256_after(
        stderr,
        "and canonical identity ",
        "; semantic middle-end construction remains pending; no fallback or artifact emission was entered",
        "canonical semantic MIR",
    )
}

fn canonical_sha256_after<'a>(
    stderr: &'a str,
    prefix: &str,
    trailer: &str,
    label: &str,
) -> &'a str {
    assert_eq!(
        stderr.match_indices(prefix).count(),
        1,
        "expected exactly one {label} identity diagnostic:\n{stderr}",
    );
    let suffix = stderr
        .split_once(prefix)
        .unwrap_or_else(|| panic!("missing {label} identity diagnostic:\n{stderr}"))
        .1;
    let identity = suffix
        .get(..64)
        .unwrap_or_else(|| panic!("truncated {label} identity diagnostic:\n{stderr}"));
    assert!(
        identity
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} identity is not canonical lowercase hexadecimal: {identity:?}",
    );
    assert!(
        suffix[64..].starts_with(trailer),
        "{label} identity has a non-canonical diagnostic trailer:\n{stderr}",
    );
    identity
}
