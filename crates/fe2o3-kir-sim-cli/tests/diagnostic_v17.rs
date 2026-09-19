//! Ordinary executable controls over synthetic diagnostic V17 owners.
//! These fixtures do not establish actual Rust source export, source custody,
//! physical registers, native instruction retention, or GPU execution.
#![cfg(target_os = "linux")]

#[path = "fixtures/diagnostic_kir_v17.rs"]
mod fixture;
#[path = "fixtures/diagnostic_kir_v16.rs"]
mod fixture_v16;

use fe2o3_kernel_ir::{ScalarType, Type, encode_module_v17};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const INPUTS: [[u32; 3]; 6] = [
    [0, 0, 0],
    [u32::MAX, 0, 1],
    [u32::MAX, 1, 2],
    [0x8000_0000, 0, 0x8000_0000],
    [0xaaaa_5555, 0x5555_aaaa, 19],
    [19, 23, 42],
];
const PROGRAMS: [&[u16]; 3] = [
    &[8],
    &[133, 307, 413],
    &[
        0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72,
    ],
];

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-diagnostic-v17-command-{}-{}",
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
        .arg("--diagnostic-kir-v17")
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

fn failure(output: Output, stage: &str, kind: &str, input: Option<&str>) -> Value {
    assert!(!output.status.success());
    assert!(
        output.status.code().is_some(),
        "a signal is not a typed refusal"
    );
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema"], "fe2o3-simulation-error-v1");
    assert_eq!(error["status"], "error");
    assert_eq!(error["stage"], stage);
    assert_eq!(error["kind"], kind);
    match input {
        Some(expected) => assert_eq!(error["input"], expected),
        None => assert!(error.get("input").is_none()),
    }
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );
    error
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn request(inputs: [u32; 3]) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&fixture::request(inputs)).unwrap();
    // All 264 storage bytes begin uninitialized, including two trailing canary
    // words. Successful stores initialize exactly the first 256 bytes.
    value["arguments"][3]["initialized"] = json!(format!("0x{}", "00".repeat(33)));
    serde_json::to_vec(&value).unwrap()
}

fn expected_program(profile: usize, [a, b, c]: [u32; 3]) -> u32 {
    // Independent arithmetic: no descriptor decoder or program evaluator.
    match profile {
        0 => a,
        1 => b ^ ((a ^ b) & c),
        2 => {
            let selected = ((a ^ b) & c) | b;
            let reused = selected.wrapping_add(c).wrapping_sub(a) ^ b;
            let masked = (reused | a) & c;
            masked.wrapping_add(a).wrapping_sub(b) ^ c
        }
        _ => panic!("unknown synthetic profile"),
    }
}

fn assert_result(result: &Value, inputs: [u32; 3], profile: usize, used: bool) {
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
    assert_eq!(
        result["target_profile"]["identity"],
        "amdgpu_64_little_endian_v1"
    );
    assert_eq!(result["target_profile"]["index_bits"], 64);
    assert_eq!(result["counts"]["arguments"], 4);
    assert_eq!(result["counts"]["shared_buffers"], 0);
    assert_eq!(result["counts"]["invocations_executed"], 64);
    assert_eq!(result["counts"]["workgroups_visited"], 1);
    assert_eq!(result["arguments"].as_array().unwrap().len(), 4);
    assert!(result["shared_buffers"].as_array().unwrap().is_empty());
    for (index, input) in inputs.into_iter().enumerate() {
        assert_eq!(
            result["arguments"][index],
            json!({
                "kind": "scalar", "type": "u32", "bits": format!("0x{input:08x}"),
            })
        );
    }
    let stored = if used {
        expected_program(profile, inputs)
    } else {
        inputs[0]
    };
    let expected = fixture::output(stored);
    assert_eq!(expected.len(), 264);
    assert_eq!(&expected[256..], &[0x5a; 8]);
    assert_eq!(
        result["arguments"][3],
        json!({
            "kind": "buffer",
            "value": {
                "element": "u32", "access": "read_write", "alignment": 4,
                "bytes": format!("0x{}", hex(&expected)),
                "initialized": format!("0x{}00", "ff".repeat(32)),
            },
        })
    );
}

#[test]
fn ordinary_v17_one_three_sixteen_used_unused_preserve_exact_outputs_and_identities() {
    let directory = Directory::new();
    let kir = directory.path("program.kir");
    let input = directory.path("request.json");
    let mut identities = BTreeSet::new();
    let mut executions = 0;
    assert_eq!(PROGRAMS.map(|program| program.len()), [1, 3, 16]);
    assert_eq!(
        [
            expected_program(0, INPUTS[5]),
            expected_program(1, INPUTS[5]),
            expected_program(2, INPUTS[5])
        ],
        [19, 23, 12]
    );
    for (profile, descriptors) in PROGRAMS.into_iter().enumerate() {
        for used in [true, false] {
            let owner = fixture::owner(&fixture::module_with_program(
                used,
                fixture::program(descriptors),
            ));
            let identity = hex(owner.identity().digest());
            assert!(
                identities.insert(identity.clone()),
                "each synthetic owner is distinct"
            );
            fs::write(&kir, owner.canonical_bytes()).unwrap();
            for inputs in INPUTS {
                let request_bytes = request(inputs);
                fs::write(&input, &request_bytes).unwrap();
                let result = success(run(&kir, &input).output().unwrap());
                assert_eq!(result["kir"]["sha256"], identity);
                assert_eq!(
                    result["kir"]["canonical_bytes"],
                    owner.identity().canonical_length()
                );
                assert_result(&result, inputs, profile, used);
                assert_eq!(fs::read(&kir).unwrap(), owner.canonical_bytes());
                assert_eq!(fs::read(&input).unwrap(), request_bytes);
                executions += 1;
            }
        }
    }
    assert_eq!(identities.len(), 6);
    assert_eq!(executions, 36);
}

#[test]
fn v17_output_is_create_new_and_preserves_a_previously_published_result() {
    let directory = Directory::new();
    let kir = directory.path("program.kir");
    let input = directory.path("request.json");
    let output = directory.path("result.json");
    let owner = fixture::owner(&fixture::module_with_program(
        true,
        fixture::program(PROGRAMS[2]),
    ));
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    fs::write(&input, request(INPUTS[5])).unwrap();
    let first = run(&kir, &input)
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
    assert!(first.stderr.is_empty());
    let saved = fs::read(&output).unwrap();
    let result: Value = serde_json::from_slice(&saved).unwrap();
    assert_result(&result, INPUTS[5], 2, true);
    assert_eq!(result["kir"]["sha256"], hex(owner.identity().digest()));
    // A different valid request must not overwrite the first result either.
    fs::write(&input, request(INPUTS[0])).unwrap();
    failure(
        run(&kir, &input)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap(),
        "output",
        "output_already_exists",
        None,
    );
    assert_eq!(fs::read(&output).unwrap(), saved);
}

#[test]
fn all_nine_v17_schedule_options_refuse_before_input_or_output_io() {
    let directory = Directory::new();
    let kir = directory.path("absent.kir");
    let input = directory.path("absent-request.json");
    let output = directory.path("must-not-be-created.json");
    let schedule = directory.path("must-not-be-created.schedule");
    let schedule_path = schedule.to_str().unwrap();
    for extra in [
        vec!["--record-canonical-schedule", schedule_path],
        vec!["--record-seeded-schedule", schedule_path],
        vec!["--replay-schedule", schedule_path],
        vec!["--explore-seeded-schedules", "1"],
        vec!["--reduce-failure"],
        vec!["--replay-failure-reduction", schedule_path],
        vec!["--schedule-seed", "0"],
        vec!["--schedule-max-decisions", "1"],
        vec!["--exploration-max-retained-decisions", "1"],
    ] {
        let error = failure(
            run(&kir, &input)
                .arg("--output")
                .arg(&output)
                .args(&extra)
                .output()
                .unwrap(),
            "arguments",
            "schedule_input_unsupported",
            None,
        );
        assert!(error["message"].as_str().unwrap().contains("V17"));
        assert!(!kir.exists());
        assert!(!input.exists());
        assert!(!output.exists());
        assert!(!schedule.exists());
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn mixed_v17_and_other_canonical_or_source_bundle_selectors_refuse_before_io() {
    let directory = Directory::new();
    let first = directory.path("absent-first-input");
    let second = directory.path("absent-second-input");
    let input = directory.path("absent-request.json");
    let output = directory.path("must-not-be-created.json");
    for flag in [
        "--kir-v7",
        "--kir-v12",
        "--diagnostic-kir-v16",
        "--diagnostic-kir-v17",
        "--bundle",
        "--bundle-v5",
        "--bundle-v6",
    ] {
        for v17_first in [true, false] {
            let selectors = if v17_first {
                ["--diagnostic-kir-v17", flag]
            } else {
                [flag, "--diagnostic-kir-v17"]
            };
            failure(
                binary()
                    .arg(selectors[0])
                    .arg(&first)
                    .arg(selectors[1])
                    .arg(&second)
                    .arg("--request")
                    .arg(&input)
                    .arg("--output")
                    .arg(&output)
                    .output()
                    .unwrap(),
                "arguments",
                "invalid_command_line",
                None,
            );
            assert!(!first.exists());
            assert!(!second.exists());
            assert!(!input.exists());
            assert!(!output.exists());
        }
    }
    failure(
        binary().arg("--diagnostic-kir-v17").output().unwrap(),
        "arguments",
        "invalid_command_line",
        None,
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn v17_wrong_wire_semantics_and_launch_have_exact_typed_refusals() {
    let directory = Directory::new();
    let kir = directory.path("program.kir");
    let input = directory.path("request.json");
    let output = directory.path("must-not-be-created.json");
    let owner = fixture::owner(&fixture::module_with_program(
        true,
        fixture::program(PROGRAMS[1]),
    ));
    fs::write(&input, request(INPUTS[5])).unwrap();
    for version in [1_u16, 7, 11, 12, 15, 16] {
        let mut bytes = owner.canonical_bytes().to_vec();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        fs::write(&kir, bytes).unwrap();
        failure(
            run(&kir, &input)
                .arg("--output")
                .arg(&output)
                .output()
                .unwrap(),
            "kir_admission",
            "kir_v17_wrong_version",
            Some("kir_v17"),
        );
        assert!(!output.exists());
    }
    // Also reject a genuine synthetic V16 owner, not just a changed header.
    fs::write(
        &kir,
        fixture_v16::owner(&fixture_v16::module(true)).canonical_bytes(),
    )
    .unwrap();
    failure(
        run(&kir, &input).output().unwrap(),
        "kir_admission",
        "kir_v17_wrong_version",
        Some("kir_v17"),
    );
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    failure(
        binary()
            .arg("--diagnostic-kir-v16")
            .arg(&kir)
            .arg("--request")
            .arg(&input)
            .output()
            .unwrap(),
        "kir_admission",
        "kir_v16_wrong_version",
        Some("kir_v16"),
    );

    let mut invalid = fixture::module_with_program(true, fixture::program(PROGRAMS[1]));
    invalid.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::I32);
    fs::write(&kir, encode_module_v17(&invalid).unwrap()).unwrap();
    failure(
        run(&kir, &input).output().unwrap(),
        "kir_admission",
        "kir_v17_verification_failed",
        Some("kir_v17"),
    );

    fs::write(&kir, owner.canonical_bytes()).unwrap();
    let mut wrong_launch: Value = serde_json::from_slice(&request(INPUTS[5])).unwrap();
    wrong_launch["workgroup"] = json!([32, 1, 1]);
    fs::write(&input, serde_json::to_vec(&wrong_launch).unwrap()).unwrap();
    failure(
        run(&kir, &input)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap(),
        "preflight",
        "preflight_workgroup_mismatch",
        None,
    );
    assert!(!output.exists());
}

#[test]
fn v17_request_parser_preserves_exact_error_kinds_and_no_output() {
    let directory = Directory::new();
    let kir = directory.path("program.kir");
    let input = directory.path("request.json");
    let output = directory.path("must-not-be-created.json");
    let owner = fixture::owner(&fixture::module_with_program(
        true,
        fixture::program(PROGRAMS[0]),
    ));
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    let original = String::from_utf8(request(INPUTS[5])).unwrap();
    let duplicate = original.replacen('{', "{\"kernel\":\"program\",", 1);
    let mut cases = vec![
        (b"{".to_vec(), "request_json_syntax"),
        (duplicate.into_bytes(), "request_json_duplicate_field"),
    ];
    for (field, replacement, kind) in [
        ("unexpected", json!(true), "request_json_unknown_field"),
        ("schema", Value::Null, "request_json_null"),
        (
            "schema",
            json!("unsupported-request-v99"),
            "request_schema_unsupported",
        ),
    ] {
        let mut malformed: Value = serde_json::from_str(&original).unwrap();
        malformed[field] = replacement;
        cases.push((serde_json::to_vec(&malformed).unwrap(), kind));
    }
    let mut scalar: Value = serde_json::from_str(&original).unwrap();
    scalar["arguments"][0]["bits"] = json!("0x100000000");
    cases.push((
        serde_json::to_vec(&scalar).unwrap(),
        "request_scalar_bits_invalid",
    ));
    let mut initialization: Value = serde_json::from_str(&original).unwrap();
    initialization["arguments"][3]["initialized"] = json!("0x00");
    cases.push((
        serde_json::to_vec(&initialization).unwrap(),
        "request_initialization_invalid",
    ));
    for (bytes, kind) in cases {
        fs::write(&input, &bytes).unwrap();
        failure(
            run(&kir, &input)
                .arg("--output")
                .arg(&output)
                .output()
                .unwrap(),
            "request",
            kind,
            None,
        );
        assert_eq!(fs::read(&input).unwrap(), bytes);
        assert_eq!(fs::read(&kir).unwrap(), owner.canonical_bytes());
        assert!(!output.exists());
    }
}
