use super::{ScratchTarget, workspace};
use serde_json::Value;
use std::process::Command;

const REPORT: &str = "fe2o3 checked-u32-prefix extraction: ";

fn run(target: &ScratchTarget, features: &str) -> std::process::Output {
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(workspace())
        .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_production_extraction_fixture")
        .env("FE2O3_EXTRACT_CHECKED_U32_PREFIX_V1", "1")
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS").env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Copt-level=3 -Cpanic=abort -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32")
        .args(["check", "--locked", "--offline", "-Zbuild-std=core", "-p", "fe2o3-production-extraction-fixture",
            "--features", features, "--target", "amdgcn-amd-amdhsa", "--target-dir"])
        .arg(target.path());
    for variable in [
        "FE2O3_EXTRACT_RANKED_MEMORY_V1",
        "FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
        "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V2",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V3",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V4",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V5",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6",
    ] {
        command.env_remove(variable);
    }
    let result = command.output().expect("execute live AMD rustc extraction");
    eprintln!(
        "checked-u32 case={features} status={}\nstdout:\n{}\nstderr:\n{}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn assert_report(report: &Value, origin: usize) {
    assert_eq!(report["schema"], 1);
    assert_eq!(report["canonical_kir_version"], 8);
    assert_ne!(report["root"], report["function"]);
    assert_eq!(report["origin"]["argument"], origin);
    assert_eq!(report["literal"], 17);
    assert_eq!(report["first_entry_only"], true);
    for field in [
        "continuation_verified",
        "normalization_adapters_proved",
        "authenticates_compiler_execution",
        "artifact_or_launch_authority",
    ] {
        assert_eq!(report[field], false);
    }
    for field in ["semantic_mir_sha256", "kir_identity_sha256"] {
        let sha = report[field].as_str().unwrap();
        assert_eq!(sha.len(), 64);
        assert!(
            sha.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_ne!(sha, "0".repeat(64));
    }
    assert!(report["kir_canonical_length"].as_u64().unwrap() > 0);
    assert_eq!(
        report["define_event"].as_u64().unwrap(),
        report["use_event"].as_u64().unwrap() + 1
    );
    assert_ne!(report["kir_value"], report["kir_overflow"]);
    assert_ne!(report["kir_operand"], report["kir_value"]);
    let arguments = report["arguments"].as_array().unwrap();
    assert_eq!(arguments.len(), 2);
    for (index, argument) in arguments.iter().enumerate() {
        assert_eq!(argument["argument"], index);
    }
    assert_ne!(
        arguments[0]["semantic_local"],
        arguments[1]["semantic_local"]
    );
    assert_ne!(arguments[0]["kir_value"], arguments[1]["kir_value"]);
    let kinds = report["source_prefix_kinds"].as_array().unwrap();
    assert_eq!(
        kinds.len() as u64,
        report["source_statement"].as_u64().unwrap()
    );
    assert!(
        kinds
            .iter()
            .all(|kind| matches!(kind.as_str(), Some("nop" | "copy" | "constant")))
    );
    let evaluations = report["conditional_evaluations"].as_array().unwrap();
    assert_eq!(evaluations.len(), 4);
    for (row, base) in evaluations
        .iter()
        .zip([0u32, u32::MAX, u32::MAX - 16, 0x8000_0000])
    {
        assert_eq!(
            row["arguments"],
            serde_json::json!([base, base.wrapping_add(1)])
        );
        let (value, overflow) = base.wrapping_add(origin as u32).overflowing_add(17);
        assert_eq!(row["value"], value);
        assert_eq!(row["overflow"], overflow);
    }
}

#[test]
#[ignore = "requires pinned nightly rustc-dev/rust-src and compile-only AMD target"]
fn genuine_checked_u32_prefix_extraction_uses_actual_sources_and_rejects_unsupported_profiles() {
    let target = ScratchTarget::new();
    let mut reports = Vec::new();
    for (features, origin) in [("checked-u32-prefix", 0), ("checked-u32-prefix-second", 1)] {
        let result = run(&target, features);
        assert!(
            result.status.success(),
            "genuine positive extraction failed: {features}"
        );
        let stderr = String::from_utf8(result.stderr).unwrap();
        let rows = stderr
            .lines()
            .filter_map(|line| line.strip_prefix(REPORT))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1, "one genuine extraction report");
        let report: Value = serde_json::from_str(rows[0]).unwrap();
        assert_report(&report, origin);
        assert_eq!(
            report["source_prefix_kinds"],
            if origin == 0 {
                serde_json::json!(["copy", "copy"])
            } else {
                serde_json::json!([])
            },
            "pinned compiler must exercise both nonempty and empty prefixes"
        );
        reports.push(report);
    }
    assert_ne!(
        reports[0]["semantic_mir_sha256"],
        reports[1]["semantic_mir_sha256"]
    );
    assert_ne!(
        reports[0]["kir_identity_sha256"],
        reports[1]["kir_identity_sha256"]
    );
    for (features, rejection) in [
        ("checked-u32-prefix-none", "MissingEntryHelperCheckedAdd"),
        (
            "checked-u32-prefix-ambiguous",
            "AmbiguousEntryHelperCheckedAdd",
        ),
        ("checked-u32-prefix,multi-root-ownership", "RootCount"),
        (
            "checked-u32-prefix-unsupported",
            "unsupported checked-u32 entry prefix: Source",
        ),
    ] {
        let result = run(&target, features);
        assert!(!result.status.success(), "must reject {features}");
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(
            stderr.contains(rejection),
            "wrong rejection for {features}: {stderr}"
        );
        assert!(
            !stderr.contains(REPORT),
            "rejection must not report accepted relation"
        );
    }
    let mut directories = vec![target.path().to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let ty = entry.file_type().unwrap();
            if ty.is_dir() {
                directories.push(entry.path());
            }
            assert!(
                !matches!(
                    entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("hsaco" | "handoff" | "fe2sim" | "ll")
                ),
                "no artifact publication from extraction"
            );
        }
    }
}
