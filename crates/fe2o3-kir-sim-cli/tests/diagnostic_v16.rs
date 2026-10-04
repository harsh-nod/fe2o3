//! Ordinary executable controls; the actual-source exporter ladder is separate.
#![cfg(target_os = "linux")]

#[path = "fixtures/diagnostic_kir_v16.rs"]
mod fixture;

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-diagnostic-v16-command-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn binary() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim"));
    command.env_clear();
    command
}
fn run(kir: &Path, request: &Path) -> Command {
    let mut command = binary();
    command
        .arg("--diagnostic-kir-v16")
        .arg(kir)
        .arg("--request")
        .arg(request);
    command
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}
fn failure(output: Output) -> Value {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    serde_json::from_slice(&output.stderr).unwrap()
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

#[test]
fn ordinary_unscheduled_v16_runs_used_and_unused_regions_with_exact_identity() {
    let directory = Directory::new();
    let kir = directory.path("program.kir");
    let request = directory.path("request.json");
    for used in [true, false] {
        let owner = fixture::owner(&fixture::module(used));
        fs::write(&kir, owner.canonical_bytes()).unwrap();
        for inputs in [[7, 11, 13], [u32::MAX, 0, 1], [0xa5a5_a5a5, 0x5a5a_5a5a, 2]] {
            fs::write(&request, fixture::request(inputs)).unwrap();
            let result = success(run(&kir, &request).output().unwrap());
            assert_eq!(result["schema"], "fe2o3-simulation-result-v1");
            assert_eq!(result["status"], "ok");
            assert_eq!(result["authority"], "observation_only");
            assert_eq!(result["simulated"], true);
            for flag in [
                "hardware_observed",
                "hardware_validation",
                "performance_prediction",
            ] {
                assert_eq!(result[flag], false);
            }
            assert_eq!(result["kir"]["sha256"], hex(owner.identity().digest()));
            assert_eq!(
                result["kir"]["canonical_bytes"],
                owner.identity().canonical_length()
            );
            assert_eq!(result["counts"]["invocations_executed"], 64);
            assert_eq!(
                result["arguments"][3]["value"]["bytes"],
                format!("0x{}", hex(&fixture::expected_output(inputs, used)))
            );
        }
    }
}

#[test]
fn diagnostic_output_uses_existing_create_new_publication() {
    let directory = Directory::new();
    let kir = directory.path("program.kir");
    let request = directory.path("request.json");
    let output = directory.path("result.json");
    fs::write(
        &kir,
        fixture::owner(&fixture::module(true)).canonical_bytes(),
    )
    .unwrap();
    fs::write(&request, fixture::request([7, 11, 13])).unwrap();
    let first = run(&kir, &request)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stdout.is_empty());
    let saved = fs::read(&output).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&saved).unwrap()["status"],
        "ok"
    );
    let error = failure(
        run(&kir, &request)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap(),
    );
    assert_eq!(error["kind"], "output_already_exists");
    assert_eq!(fs::read(&output).unwrap(), saved);
}

#[test]
fn every_schedule_flag_is_refused_before_any_command_input_or_output() {
    for extra in [
        vec!["--record-canonical-schedule", "/missing-output"],
        vec!["--record-seeded-schedule", "/missing-output"],
        vec!["--replay-schedule", "/missing"],
        vec!["--explore-seeded-schedules", "1"],
        vec!["--reduce-failure"],
        vec!["--replay-failure-reduction", "/missing"],
        vec!["--schedule-seed", "0"],
        vec!["--schedule-max-decisions", "1"],
        vec!["--exploration-max-retained-decisions", "1"],
    ] {
        let error = failure(
            run(Path::new("/missing-kir"), Path::new("/missing-request"))
                .args(extra)
                .output()
                .unwrap(),
        );
        assert_eq!(error["stage"], "arguments");
        assert_eq!(error["kind"], "schedule_input_unsupported");
    }
    for flag in [
        "--kir-v7",
        "--kir-v12",
        "--diagnostic-kir-v16",
        "--bundle",
        "--bundle-v5",
        "--bundle-v6",
    ] {
        let error = failure(
            run(Path::new("/missing-kir"), Path::new("/missing-request"))
                .args([flag, "/another-missing-input"])
                .output()
                .unwrap(),
        );
        assert_eq!(error["stage"], "arguments");
        assert_eq!(error["kind"], "invalid_command_line");
    }
    assert_eq!(
        failure(binary().arg("--diagnostic-kir-v16").output().unwrap())["kind"],
        "invalid_command_line"
    );
}

#[test]
fn wrong_wire_versions_and_bad_launches_are_truthfully_classified() {
    let directory = Directory::new();
    let kir = directory.path("program.kir");
    let request = directory.path("request.json");
    let owner = fixture::owner(&fixture::module(true));
    fs::write(&request, fixture::request([7, 11, 13])).unwrap();
    for version in [7_u16, 11, 12, 15] {
        let mut bytes = owner.canonical_bytes().to_vec();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        fs::write(&kir, bytes).unwrap();
        let error = failure(run(&kir, &request).output().unwrap());
        assert_eq!(error["input"], "kir_v16");
        assert_eq!(error["stage"], "kir_admission");
        assert_eq!(error["kind"], "kir_v16_wrong_version");
    }
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    for old_flag in ["--kir-v7", "--kir-v12", "--bundle-v6"] {
        assert!(
            !binary()
                .arg(old_flag)
                .arg(&kir)
                .arg("--request")
                .arg(&request)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let mut bad: Value = serde_json::from_slice(&fixture::request([7, 11, 13])).unwrap();
    bad["workgroup"] = serde_json::json!([32, 1, 1]);
    fs::write(&request, serde_json::to_vec(&bad).unwrap()).unwrap();
    let error = failure(run(&kir, &request).output().unwrap());
    assert_eq!(error["stage"], "preflight");
    assert_eq!(error["kind"], "preflight_workgroup_mismatch");
}
