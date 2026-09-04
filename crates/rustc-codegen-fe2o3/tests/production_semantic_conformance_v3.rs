use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_kernel_ir::ScalarType;
use fe2o3_sim_differential::{
    ExactBufferExpectationV3, ExactBufferUnavailableV3, ProductionSemanticCaseV3,
    ProductionSemanticConformanceErrorV3, run_production_semantic_conformance_v3,
    semantic_differential_capabilities_v2,
};
use serde_json::{Value, json};

const LANES: usize = 64;

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-semantic-conformance-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create production conformance scratch directory");
        Self { path }
    }
}

impl Drop for Scratch {
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

fn export_feature(scratch: &Scratch, feature: &str) -> Output {
    const POISONED_WRAPPER: &str = "/fe2o3-poisoned-caller-wrapper-must-not-run";

    Command::new(env!("CARGO_BIN_EXE_fe2o3-export-sim"))
        .current_dir(workspace())
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS")
        .env("RUSTC_WRAPPER", POISONED_WRAPPER)
        .env("CARGO_BUILD_RUSTC_WRAPPER", POISONED_WRAPPER)
        .env("RUSTC_WORKSPACE_WRAPPER", POISONED_WRAPPER)
        .env("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", POISONED_WRAPPER)
        .env_remove("FE2O3_EXTRACT_CRATE_V1")
        .env_remove("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1")
        .env_remove("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V2")
        .env_remove("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V3")
        .env_remove("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V4")
        .env_remove("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V5")
        .env_remove("FE2O3_EXTRACT_RANKED_MEMORY_V1")
        .env_remove("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1")
        .env_remove("FE2O3_EXTRACT_GFX942_LLVM_PATH_V1")
        .env_remove("FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1")
        .env_remove("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1")
        .arg("--crate")
        .arg("fe2o3_production_semantic_conformance_fixture")
        .arg("--output")
        .arg(bundle_path(scratch, feature))
        .args([
            "--target",
            "gfx942",
            "--bundle-version",
            "5",
            "--target-dir",
        ])
        .arg(scratch.path.join("fixture-target"))
        .args([
            "--",
            "--package",
            "fe2o3-production-semantic-conformance-fixture",
            "--features",
            feature,
            "--lib",
        ])
        .output()
        .expect("run production Bundle V5 exporter")
}

fn require_export(scratch: &Scratch, feature: &str) -> PathBuf {
    let output = export_feature(scratch, feature);
    assert!(
        output.status.success(),
        "feature {feature} did not export to Bundle V5:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("export diagnostic is UTF-8");
    assert!(
        stderr.contains("exact same-module KIR V10")
            && stderr.contains("compiler_execution=extraction_only_unavailable")
            && stderr.contains("authority false"),
        "feature {feature} overclaimed or omitted the V5 custody contract:\n{stderr}"
    );
    bundle_path(scratch, feature)
}

fn bundle_path(scratch: &Scratch, feature: &str) -> PathBuf {
    scratch.path.join(format!("{feature}.fe2sim"))
}

fn write_request(scratch: &Scratch, case_id: &str, kernel: &str, arguments: Value) -> PathBuf {
    let path = scratch.path.join(format!("{case_id}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema": "fe2o3-simulation-request-v1",
            "kernel": kernel,
            "grid": [64, 1, 1],
            "workgroup": [64, 1, 1],
            "arguments": arguments,
        }))
        .expect("encode simulation request"),
    )
    .expect("write simulation request");
    path
}

fn buffer_json(element: &str, bytes: &[u8], alignment: usize) -> Value {
    json!({
        "kind": "buffer",
        "element": element,
        "access": "read_write",
        "alignment": alignment,
        "bytes": format!("0x{}", hex(bytes)),
    })
}

fn scalar_json(element: &str, bits: u128, element_bytes: usize) -> Value {
    let width = element_bytes * 2;
    json!({
        "kind": "scalar",
        "type": element,
        "bits": format!("0x{bits:0width$x}"),
    })
}

fn exact_scalar_output_case(
    scratch: &Scratch,
    bundle: &Path,
    case_id: &str,
    kernel: &str,
    element_name: &str,
    element: ScalarType,
    element_bytes: usize,
    left: u128,
    right: u128,
    expected: u128,
) {
    let zeroes = vec![0_u8; LANES * element_bytes];
    let request = write_request(
        scratch,
        case_id,
        kernel,
        json!([
            scalar_json(element_name, left, element_bytes),
            scalar_json(element_name, right, element_bytes),
            buffer_json(element_name, &zeroes, element_bytes),
        ]),
    );
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v5(bundle, &request)
        .expect("strictly admit compiler-produced Bundle V5 and request");
    let expected = repeated_scalar_bytes(expected, element_bytes);
    let initialized = vec![true; expected.len()];
    let outputs = [ExactBufferExpectationV3 {
        argument_ordinal: 2,
        element,
        bytes: &expected,
        initialized: &initialized,
    }];
    let report = run_production_semantic_conformance_v3(
        &admitted,
        ProductionSemanticCaseV3 {
            case_id,
            outputs: &outputs,
        },
    )
    .expect("run exact production semantic conformance case");
    assert_eq!(report.status, "agreement", "case {case_id}");
    assert!(!report.hardware_observed);
    assert!(!report.performance_prediction);
    assert_eq!(report.bundle_version, 5);
    assert_eq!(report.kir_version, 10);
    assert_eq!(report.expected_bytes, LANES * element_bytes);
}

fn repeated_scalar_bytes(bits: u128, element_bytes: usize) -> Vec<u8> {
    let scalar = bits.to_le_bytes();
    let mut output = Vec::with_capacity(LANES * element_bytes);
    for _ in 0..LANES {
        output.extend_from_slice(&scalar[..element_bytes]);
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn next_generated(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn signed_less(left: u128, right: u128, bits: u32) -> bool {
    let sign = 1_u128 << (bits - 1);
    (left ^ sign) < (right ^ sign)
}

#[derive(Clone, Copy)]
struct IntegerProfile {
    feature: &'static str,
    kernel: &'static str,
    element_name: &'static str,
    element: ScalarType,
    bits: u32,
    signed: bool,
}

const INTEGER_PROFILES: [IntegerProfile; 8] = [
    integer("integer-i8", "integer_i8", "i8", ScalarType::I8, 8, true),
    integer(
        "integer-i16",
        "integer_i16",
        "i16",
        ScalarType::I16,
        16,
        true,
    ),
    integer(
        "integer-i32",
        "integer_i32",
        "i32",
        ScalarType::I32,
        32,
        true,
    ),
    integer(
        "integer-i64",
        "integer_i64",
        "i64",
        ScalarType::I64,
        64,
        true,
    ),
    integer("integer-u8", "integer_u8", "u8", ScalarType::U8, 8, false),
    integer(
        "integer-u16",
        "integer_u16",
        "u16",
        ScalarType::U16,
        16,
        false,
    ),
    integer(
        "integer-u32",
        "integer_u32",
        "u32",
        ScalarType::U32,
        32,
        false,
    ),
    integer(
        "integer-u64",
        "integer_u64",
        "u64",
        ScalarType::U64,
        64,
        false,
    ),
];

const fn integer(
    feature: &'static str,
    kernel: &'static str,
    element_name: &'static str,
    element: ScalarType,
    bits: u32,
    signed: bool,
) -> IntegerProfile {
    IntegerProfile {
        feature,
        kernel,
        element_name,
        element,
        bits,
        signed,
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_bundle_v5_integer_widths_and_signedness_match_generated_cpu_cases() {
    let scratch = Scratch::new();
    for (profile_index, profile) in INTEGER_PROFILES.iter().copied().enumerate() {
        let bundle = require_export(&scratch, profile.feature);
        let mask = if profile.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << profile.bits) - 1
        };
        let mut cases = vec![(mask, 1_u128)];
        let mut state = 0x216_c100_0000_0001_u64 ^ profile_index as u64;
        for _ in 0..3 {
            let left = u128::from(next_generated(&mut state)) & mask;
            let right = u128::from(next_generated(&mut state)) & mask;
            cases.push((left, right));
        }
        for (case_index, (left, right)) in cases.into_iter().enumerate() {
            let less = if profile.signed {
                signed_less(left, right, profile.bits)
            } else {
                left < right
            };
            let expected = if less { left ^ right } else { left & right };
            exact_scalar_output_case(
                &scratch,
                &bundle,
                &format!("{}-{case_index}", profile.feature),
                profile.kernel,
                profile.element_name,
                profile.element,
                profile.bits as usize / 8,
                left,
                right,
                expected,
            );
        }
    }
}

#[derive(Clone, Copy)]
struct FloatProfile {
    feature: &'static str,
    kernel: &'static str,
    element_name: &'static str,
    element: ScalarType,
    element_bytes: usize,
    cases: &'static [(u128, u128, u128)],
}

const F32_CASES: [(u128, u128, u128); 9] = [
    (0x3f80_0000, 0x3f80_0000, 0x4000_0000),
    (0x0000_0001, 0x0000_0001, 0x0000_0002),
    (0x007f_ffff, 0, 0x007f_ffff),
    (0x3f80_0000, 0x3380_0000, 0x3f80_0000),
    (0x7f80_0000, 0xff80_0000, 0x7fc0_0000),
    (0x7f80_0001, 0x3f80_0000, 0x7fc0_0001),
    (0x7fc0_0001, 0x3f80_0000, 0x7fc0_0001),
    (0x8000_0000, 0x8000_0000, 0x8000_0000),
    (0, 0x8000_0000, 0),
];

const F64_CASES: [(u128, u128, u128); 9] = [
    (
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x4000_0000_0000_0000,
    ),
    (1, 1, 2),
    (0x000f_ffff_ffff_ffff, 0, 0x000f_ffff_ffff_ffff),
    (
        0x3ff0_0000_0000_0000,
        0x3ca0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
    ),
    (
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0000,
    ),
    (
        0x7ff0_0000_0000_0001,
        0x3ff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
    ),
    (
        0x7ff8_0000_0000_0001,
        0x3ff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
    ),
    (
        0x8000_0000_0000_0000,
        0x8000_0000_0000_0000,
        0x8000_0000_0000_0000,
    ),
    (0, 0x8000_0000_0000_0000, 0),
];

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_bundle_v5_float_corner_tables_match_exact_cpu_bits() {
    let scratch = Scratch::new();
    let profiles = [
        FloatProfile {
            feature: "float-f32",
            kernel: "float_f32",
            element_name: "f32",
            element: ScalarType::F32,
            element_bytes: 4,
            cases: &F32_CASES,
        },
        FloatProfile {
            feature: "float-f64",
            kernel: "float_f64",
            element_name: "f64",
            element: ScalarType::F64,
            element_bytes: 8,
            cases: &F64_CASES,
        },
    ];
    for profile in profiles {
        let bundle = require_export(&scratch, profile.feature);
        for (case_index, &(left, right, expected)) in profile.cases.iter().enumerate() {
            exact_scalar_output_case(
                &scratch,
                &bundle,
                &format!("{}-{case_index}", profile.feature),
                profile.kernel,
                profile.element_name,
                profile.element,
                profile.element_bytes,
                left,
                right,
                expected,
            );
        }
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_bundle_v5_checked_output_bounds_and_hostile_expectations_are_typed() {
    let scratch = Scratch::new();
    let bundle = require_export(&scratch, "bounds-output");
    let zeroes = vec![0_u8; LANES * 4];
    let request = write_request(
        &scratch,
        "bounds-output",
        "bounds_output",
        json!([
            scalar_json("u32", 0xa5a5_5a5a, 4),
            buffer_json("u32", &zeroes, 4),
        ]),
    );
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v5(&bundle, &request)
        .expect("admit checked output bounds fixture");
    let expected = repeated_scalar_bytes(0xa5a5_5a5a, 4);
    let initialized = vec![true; expected.len()];
    let exact = [ExactBufferExpectationV3 {
        argument_ordinal: 1,
        element: ScalarType::U32,
        bytes: &expected,
        initialized: &initialized,
    }];
    assert_eq!(
        run_production_semantic_conformance_v3(
            &admitted,
            ProductionSemanticCaseV3 {
                case_id: "bounds-output-exact",
                outputs: &exact,
            },
        )
        .unwrap()
        .status,
        "agreement"
    );

    let wrong_type = [ExactBufferExpectationV3 {
        element: ScalarType::I32,
        ..exact[0]
    }];
    let report = run_production_semantic_conformance_v3(
        &admitted,
        ProductionSemanticCaseV3 {
            case_id: "bounds-output-wrong-type",
            outputs: &wrong_type,
        },
    )
    .unwrap();
    assert_eq!(report.status, "mismatch");
    assert_eq!(
        report.outputs[0].unavailable,
        Some(ExactBufferUnavailableV3::ScalarTypeMismatch)
    );

    let missing = [ExactBufferExpectationV3 {
        argument_ordinal: 99,
        ..exact[0]
    }];
    let report = run_production_semantic_conformance_v3(
        &admitted,
        ProductionSemanticCaseV3 {
            case_id: "bounds-output-missing",
            outputs: &missing,
        },
    )
    .unwrap();
    assert_eq!(
        report.outputs[0].unavailable,
        Some(ExactBufferUnavailableV3::MissingArgument)
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_producer_boundaries_and_cross_artifact_inputs_fail_closed() {
    let scratch = Scratch::new();
    for (feature, diagnostic) in [
        ("atomic-u32", "call terminator"),
        ("unsupported-memory", "MemoryOffsetFrom"),
        ("unsupported-i128", "argument 1 has unsupported type `i128`"),
    ] {
        let output = export_feature(&scratch, feature);
        assert!(!output.status.success(), "{feature} unexpectedly exported");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{feature} omitted typed fail-closed diagnostic {diagnostic:?}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let switch_bundle = require_export(&scratch, "switch-u32");
    let zeroes = vec![0_u8; LANES * 4];
    let switch_request = write_request(
        &scratch,
        "switch-u32-request",
        "switch_u32",
        json!([scalar_json("u32", 0, 4), buffer_json("u32", &zeroes, 4),]),
    );
    let admitted =
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v5(&switch_bundle, &switch_request)
            .expect("switch Bundle V5 admission retains no execution claim");
    let expected = repeated_scalar_bytes(11, 4);
    let initialized = vec![true; expected.len()];
    let outputs = [ExactBufferExpectationV3 {
        argument_ordinal: 1,
        element: ScalarType::U32,
        bytes: &expected,
        initialized: &initialized,
    }];
    let report = run_production_semantic_conformance_v3(
        &admitted,
        ProductionSemanticCaseV3 {
            case_id: "switch-u32-exact",
            outputs: &outputs,
        },
    )
    .expect("run exact ordinary switch conformance case");
    assert_eq!(report.status, "agreement");
    assert!(!report.hardware_observed);
    assert!(!report.performance_prediction);
    assert_eq!(report.bundle_version, 5);
    assert_eq!(report.kir_version, 10);
    assert_eq!(report.expected_bytes, LANES * 4);

    let integer_bundle = require_export(&scratch, "integer-u32");
    let cross_artifact =
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v5(&integer_bundle, &switch_request)
            .expect("request syntax admission precedes exact kernel preflight");
    assert!(matches!(
        run_production_semantic_conformance_v3(
            &cross_artifact,
            ProductionSemanticCaseV3 {
                case_id: "cross-artifact-request",
                outputs: &outputs,
            },
        ),
        Err(ProductionSemanticConformanceErrorV3::Simulation(_))
    ));
    let mut corrupted = std::fs::read(&integer_bundle).unwrap();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    let corrupted_path = scratch.path.join("corrupted.fe2sim");
    std::fs::write(&corrupted_path, corrupted).unwrap();
    assert!(
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v5(&corrupted_path, &switch_request,)
            .is_err(),
        "corrupted Bundle V5 bytes must fail strict admission"
    );

    let legacy = semantic_differential_capabilities_v2();
    assert_eq!(
        legacy.schema,
        "fe2o3-sim-semantic-differential-capabilities-v2"
    );
    assert_eq!(legacy.exact_float_types, ["f16", "bf16", "f32", "f64"]);
}
