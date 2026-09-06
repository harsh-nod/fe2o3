use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CastKind, OperationKind, ScalarType, Type,
    VerifiedCanonicalKernelIrV11, VerifiedSimulationBundleV6,
};
use fe2o3_sim_differential::{
    ExactBufferExpectationV4, ExactBufferUnavailableV4, ProductionSemanticCaseV4,
    ProductionSemanticConformanceErrorV4, run_production_semantic_conformance_v4,
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
        .env_remove("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6")
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
            "6",
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
        .expect("run production Bundle V6 exporter")
}

fn require_export(scratch: &Scratch, feature: &str) -> PathBuf {
    let output = export_feature(scratch, feature);
    assert!(
        output.status.success(),
        "feature {feature} did not export to Bundle V6:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("export diagnostic is UTF-8");
    assert!(
        stderr.contains("exact same-module KIR V11")
            && stderr.contains("compiler_execution=extraction_only_unavailable")
            && stderr.contains("authority false"),
        "feature {feature} overclaimed or omitted the V6 custody contract:\n{stderr}"
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

#[allow(clippy::too_many_arguments)]
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
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(bundle, &request)
        .expect("strictly admit compiler-produced Bundle V6 and request");
    let expected = repeated_scalar_bytes(expected, element_bytes);
    let initialized = vec![true; expected.len()];
    let outputs = [ExactBufferExpectationV4 {
        argument_ordinal: 2,
        element,
        bytes: &expected,
        initialized: &initialized,
    }];
    let report = run_production_semantic_conformance_v4(
        &admitted,
        ProductionSemanticCaseV4 {
            case_id,
            outputs: &outputs,
        },
    )
    .expect("run exact production semantic conformance case");
    assert_eq!(report.status, "agreement", "case {case_id}");
    assert!(!report.hardware_observed);
    assert!(!report.performance_prediction);
    assert_eq!(report.bundle_version, 6);
    assert_eq!(report.kir_version, 11);
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
fn ordinary_bundle_v6_integer_widths_and_signedness_match_generated_cpu_cases() {
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
fn ordinary_bundle_v6_float_corner_tables_match_exact_cpu_bits() {
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
fn ordinary_bundle_v6_checked_output_bounds_and_hostile_expectations_are_typed() {
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
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&bundle, &request)
        .expect("admit checked output bounds fixture");
    let expected = repeated_scalar_bytes(0xa5a5_5a5a, 4);
    let initialized = vec![true; expected.len()];
    let exact = [ExactBufferExpectationV4 {
        argument_ordinal: 1,
        element: ScalarType::U32,
        bytes: &expected,
        initialized: &initialized,
    }];
    assert_eq!(
        run_production_semantic_conformance_v4(
            &admitted,
            ProductionSemanticCaseV4 {
                case_id: "bounds-output-exact",
                outputs: &exact,
            },
        )
        .unwrap()
        .status,
        "agreement"
    );

    let wrong_type = [ExactBufferExpectationV4 {
        element: ScalarType::I32,
        ..exact[0]
    }];
    let report = run_production_semantic_conformance_v4(
        &admitted,
        ProductionSemanticCaseV4 {
            case_id: "bounds-output-wrong-type",
            outputs: &wrong_type,
        },
    )
    .unwrap();
    assert_eq!(report.status, "mismatch");
    assert_eq!(
        report.outputs[0].unavailable,
        Some(ExactBufferUnavailableV4::ScalarTypeMismatch)
    );

    let missing = [ExactBufferExpectationV4 {
        argument_ordinal: 99,
        ..exact[0]
    }];
    let report = run_production_semantic_conformance_v4(
        &admitted,
        ProductionSemanticCaseV4 {
            case_id: "bounds-output-missing",
            outputs: &missing,
        },
    )
    .unwrap();
    assert_eq!(
        report.outputs[0].unavailable,
        Some(ExactBufferUnavailableV4::MissingArgument)
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_rust_nested_control_flow_executes_after_production_ssa_lowering() {
    let scratch = Scratch::new();
    let bundle = require_export(&scratch, "ssa-control-flow");
    let zeroes = vec![0_u8; LANES * 4];

    for (limit, expected_value) in [(0_u32, 0_u32), (1, 3), (5, 3), (6, 0)] {
        let case_id = format!("ssa-control-flow-{limit}");
        let request = write_request(
            &scratch,
            &case_id,
            "ssa_control_flow",
            json!([
                scalar_json("u32", u128::from(limit), 4),
                buffer_json("u32", &zeroes, 4),
            ]),
        );
        let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&bundle, &request)
            .expect("admit compiler-produced nested-control-flow bundle");
        let expected = repeated_scalar_bytes(u128::from(expected_value), 4);
        let initialized = vec![true; expected.len()];
        let outputs = [ExactBufferExpectationV4 {
            argument_ordinal: 1,
            element: ScalarType::U32,
            bytes: &expected,
            initialized: &initialized,
        }];
        let report = run_production_semantic_conformance_v4(
            &admitted,
            ProductionSemanticCaseV4 {
                case_id: &case_id,
                outputs: &outputs,
            },
        )
        .expect("execute nested loops and match in compiler-produced KIR");
        assert_eq!(report.status, "agreement", "case {case_id}");
        assert_eq!(report.bundle_version, 6);
        assert_eq!(report.kir_version, 11);
        assert!(!report.hardware_observed);
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_rust_retained_shared_borrow_reaches_v11_optimizer_and_llvm() {
    let scratch = Scratch::new();
    let bundle_path = require_export(&scratch, "retained-shared-borrow");
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(
        std::fs::read(&bundle_path).expect("read compiler-produced Bundle V6"),
    )
    .expect("revalidate compiler-produced Bundle V6");
    assert_eq!(bundle.production_kir_identity().version(), 11);
    assert_eq!(
        bundle.canonical_kir_v11_length(),
        bundle.production_kir_identity().canonical_length()
    );
    assert_eq!(
        *bundle.canonical_kir_v11_digest(),
        bundle.production_kir_identity().digest(),
    );
    let (canonical_v11, module) = VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(
        bundle.canonical_kir_v11().to_vec(),
    )
    .expect("Bundle V6 owns exact verified canonical KIR V11");
    assert_eq!(
        canonical_v11.identity().digest(),
        bundle.canonical_kir_v11_digest()
    );
    assert_eq!(
        canonical_v11.identity().canonical_length(),
        bundle.canonical_kir_v11_length(),
    );

    let function = module
        .functions
        .iter()
        .find(|function| function.id.as_str() == "retained_shared_borrow")
        .expect("retained shared-borrow KIR function");
    let operations = function
        .body
        .as_ref()
        .expect("defined retained shared-borrow body")
        .blocks
        .iter()
        .flat_map(|block| block.operations.iter())
        .collect::<Vec<_>>();
    let slot = operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            } => operation.results.first().map(|result| result.id),
            _ => None,
        })
        .expect("shared-borrow local is retained in one private u32 slot");
    assert!(operations.iter().any(|operation| {
        matches!(operation.kind, OperationKind::Store { pointer, .. } if pointer == slot)
    }));
    let restricted = operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                to: Type::Pointer(pointer),
            } if *value == slot
                && pointer.pointee.as_ref() == &Type::Scalar(ScalarType::U32)
                && pointer.address_space == AddressSpace::Private
                && pointer.access == AccessMode::ReadOnly =>
            {
                operation.results.first().map(|result| result.id)
            }
            _ => None,
        })
        .expect("shared borrow narrows the retained slot to an exact read-only view");
    assert!(operations.iter().any(|operation| {
        matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == slot)
    }));
    assert!(!operations.iter().any(|operation| {
        matches!(operation.kind, OperationKind::Store { pointer, .. } if pointer == restricted)
    }));

    let zeroes = vec![0_u8; LANES * 4];
    let request = write_request(
        &scratch,
        "retained-shared-borrow",
        "retained_shared_borrow",
        json!([
            scalar_json("u32", 0x8bad_f00d, 4),
            buffer_json("u32", &zeroes, 4),
        ]),
    );
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&bundle_path, &request)
        .expect("admit compiler-produced retained shared-borrow bundle");
    let expected = repeated_scalar_bytes(0x8bad_f00d, 4);
    let initialized = vec![true; expected.len()];
    let outputs = [ExactBufferExpectationV4 {
        argument_ordinal: 1,
        element: ScalarType::U32,
        bytes: &expected,
        initialized: &initialized,
    }];
    let report = run_production_semantic_conformance_v4(
        &admitted,
        ProductionSemanticCaseV4 {
            case_id: "retained-shared-borrow",
            outputs: &outputs,
        },
    )
    .expect("execute retained shared-borrow kernel");
    assert_eq!(report.status, "agreement");
    assert_eq!(report.bundle_version, 6);
    assert_eq!(report.kir_version, 11);

    let llvm_path = scratch.path.join("retained-shared-borrow.ll");
    let binding_path = scratch.path.join("retained-shared-borrow.crate-binding");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_semantic_conformance_fixture",
        )
        .env("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1", &binding_path)
        .env("FE2O3_EXTRACT_GFX942_LLVM_PATH_V1", &llvm_path)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-semantic-conformance-fixture",
            "--features",
            "retained-shared-borrow",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(scratch.path.join("llvm-target"))
        .arg("--lib")
        .output()
        .expect("run retained shared-borrow production gfx942 extraction");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        output.status.success()
            && stderr.contains("Kernel IR V11")
            && stderr.contains("target-KIR optimizer (7 pass(es)")
            && stderr.contains("-> gfx942:xnack- LLVM")
            && stderr.contains("artifact/launch authority false"),
        "retained shared-borrow extraction omitted V11 optimizer custody:\n{stderr}",
    );
    let llvm = std::fs::read_to_string(&llvm_path)
        .expect("production extraction emitted retained shared-borrow LLVM");
    let alloca_line = llvm
        .lines()
        .find(|line| line.contains(" = alloca i32, align 4, addrspace(5)"))
        .expect("LLVM retains the private u32 slot");
    let slot_name = alloca_line
        .split_once(" = alloca")
        .map(|(name, _)| name.trim())
        .expect("private alloca has one SSA result");
    assert!(llvm.lines().any(|line| {
        line.contains("store i32 ") && line.contains(&format!("ptr addrspace(5) {slot_name}"))
    }));
    assert!(llvm.lines().any(|line| {
        line.contains(" = load i32, ptr addrspace(5) ") && line.contains(slot_name)
    }));
    let binding = std::fs::read_to_string(&binding_path).expect("crate binding handoff");
    assert_eq!(binding.trim().len(), 64);
    assert!(binding.trim().bytes().all(|byte| byte.is_ascii_hexdigit()));
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
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&switch_bundle, &switch_request)
            .expect("switch Bundle V6 admission retains no execution claim");
    let expected = repeated_scalar_bytes(11, 4);
    let initialized = vec![true; expected.len()];
    let outputs = [ExactBufferExpectationV4 {
        argument_ordinal: 1,
        element: ScalarType::U32,
        bytes: &expected,
        initialized: &initialized,
    }];
    let report = run_production_semantic_conformance_v4(
        &admitted,
        ProductionSemanticCaseV4 {
            case_id: "switch-u32-exact",
            outputs: &outputs,
        },
    )
    .expect("run exact ordinary switch conformance case");
    assert_eq!(report.status, "agreement");
    assert!(!report.hardware_observed);
    assert!(!report.performance_prediction);
    assert_eq!(report.bundle_version, 6);
    assert_eq!(report.kir_version, 11);
    assert_eq!(report.expected_bytes, LANES * 4);

    let integer_bundle = require_export(&scratch, "integer-u32");
    let cross_artifact =
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&integer_bundle, &switch_request)
            .expect("request syntax admission precedes exact kernel preflight");
    assert!(matches!(
        run_production_semantic_conformance_v4(
            &cross_artifact,
            ProductionSemanticCaseV4 {
                case_id: "cross-artifact-request",
                outputs: &outputs,
            },
        ),
        Err(ProductionSemanticConformanceErrorV4::Simulation(_))
    ));
    let mut corrupted = std::fs::read(&integer_bundle).unwrap();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    let corrupted_path = scratch.path.join("corrupted.fe2sim");
    std::fs::write(&corrupted_path, corrupted).unwrap();
    assert!(
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&corrupted_path, &switch_request,)
            .is_err(),
        "corrupted Bundle V6 bytes must fail strict admission"
    );

    let legacy = semantic_differential_capabilities_v2();
    assert_eq!(
        legacy.schema,
        "fe2o3-sim-semantic-differential-capabilities-v2"
    );
    assert_eq!(legacy.exact_float_types, ["f16", "bf16", "f32", "f64"]);
}
