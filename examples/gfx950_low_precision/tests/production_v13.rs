#![cfg(feature = "bundle-v8-simulator")]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_gfx950_low_precision::kernel::{
    ATTENTION_TOKENS, GEMM_K, GEMM_M, GEMM_N, GFX950_BATCHES, VALUE_COLUMNS,
};
use fe2o3_gfx950_low_precision::reference::{
    LowPrecisionFormat, batched_attention_reference, batched_gemm_reference,
    deterministic_batched_matrix,
};

static NONCE: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let root = std::env::var_os("FE2O3_GFX950_LOW_PRECISION_TEST_SCRATCH")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "fe2o3-gfx950-low-precision-v13-{}-{}",
                std::process::id(),
                NONCE.fetch_add(1, Ordering::Relaxed),
            ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn hex_bytes(bytes: impl IntoIterator<Item = u8>) -> String {
    let bytes = bytes.into_iter();
    let (lower, _) = bytes.size_hint();
    let mut output = String::with_capacity(2 + 2 * lower);
    output.push_str("0x");
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn zero_f32_hex(elements: usize) -> String {
    hex_bytes((0..elements).flat_map(|_| 0.0_f32.to_le_bytes()))
}

fn buffer(element: &str, access: &str, alignment: u32, bytes: String) -> serde_json::Value {
    serde_json::json!({
        "kind": "buffer",
        "element": element,
        "access": access,
        "alignment": alignment,
        "bytes": bytes,
    })
}

fn write_gemm_request(path: &Path, symbol: &str, lhs: &[u8], rhs: &[u8]) {
    let request = serde_json::json!({
        "schema": "fe2o3-simulation-request-v1",
        "kernel": symbol,
        "grid": [1024, 1, 1],
        "workgroup": [256, 1, 1],
        "arguments": [
            buffer("u8", "read_only", 1, hex_bytes(lhs.iter().copied())),
            buffer("u8", "read_only", 1, hex_bytes(rhs.iter().copied())),
            buffer("f32", "write_only", 4, zero_f32_hex(GFX950_BATCHES * GEMM_M * GEMM_N)),
        ],
    });
    std::fs::write(path, serde_json::to_vec(&request).unwrap()).unwrap();
}

fn write_attention_request(path: &Path, symbol: &str, query: &[u8], key: &[u8], value: &[u8]) {
    let request = serde_json::json!({
        "schema": "fe2o3-simulation-request-v1",
        "kernel": symbol,
        "grid": [1024, 1, 1],
        "workgroup": [256, 1, 1],
        "arguments": [
            buffer("u8", "read_only", 1, hex_bytes(query.iter().copied())),
            buffer("u8", "read_only", 1, hex_bytes(key.iter().copied())),
            buffer("u8", "read_only", 1, hex_bytes(value.iter().copied())),
            buffer("f32", "write_only", 4, zero_f32_hex(GFX950_BATCHES * ATTENTION_TOKENS * VALUE_COLUMNS)),
        ],
    });
    std::fs::write(path, serde_json::to_vec(&request).unwrap()).unwrap();
}

fn assert_deterministic_and_close(
    actual: &[f32],
    repeated: &[f32],
    expected: &[f32],
    tolerance: f32,
) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        repeated
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        "the exact Bundle V8 request was nondeterministic",
    );
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(actual.is_finite(), "output {index} is not finite: {actual}");
        let error = (actual - expected).abs();
        assert!(
            error <= tolerance + tolerance * expected.abs(),
            "output {index}: actual={actual} expected={expected} error={error}",
        );
    }
}

fn exercise_gemm(format: LowPrecisionFormat, symbol: &str, bundle_env: &str) {
    let bundle = std::env::var_os(bundle_env)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{bundle_env} must name a genuine compiler-produced Bundle V8"));
    let lhs = deterministic_batched_matrix(format, GFX950_BATCHES, GEMM_M, GEMM_K, 1);
    let rhs = deterministic_batched_matrix(format, GFX950_BATCHES, GEMM_K, GEMM_N, 3);
    let expected = batched_gemm_reference(format, GFX950_BATCHES, &lhs, &rhs).unwrap();
    let scratch = Scratch::new();
    let request = scratch.0.join("request.json");
    write_gemm_request(&request, symbol, &lhs, &rhs);
    let actual = fe2o3_gfx950_low_precision::simulator::simulate_bundle_v8(&bundle, &request, 2)
        .expect("simulate genuine GEMM Bundle V8");
    let repeated = fe2o3_gfx950_low_precision::simulator::simulate_bundle_v8(&bundle, &request, 2)
        .expect("repeat genuine GEMM Bundle V8");
    assert_deterministic_and_close(&actual, &repeated, &expected, 1.0e-5);
}

fn exercise_attention(format: LowPrecisionFormat, symbol: &str, bundle_env: &str) {
    let bundle = std::env::var_os(bundle_env)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{bundle_env} must name a genuine compiler-produced Bundle V8"));
    let query = deterministic_batched_matrix(format, GFX950_BATCHES, ATTENTION_TOKENS, GEMM_K, 2);
    let key = deterministic_batched_matrix(format, GFX950_BATCHES, ATTENTION_TOKENS, GEMM_K, 4);
    let value =
        deterministic_batched_matrix(format, GFX950_BATCHES, ATTENTION_TOKENS, VALUE_COLUMNS, 6);
    let expected =
        batched_attention_reference(format, GFX950_BATCHES, &query, &key, &value).unwrap();
    let scratch = Scratch::new();
    let request = scratch.0.join("request.json");
    write_attention_request(&request, symbol, &query, &key, &value);
    let actual = fe2o3_gfx950_low_precision::simulator::simulate_bundle_v8(&bundle, &request, 3)
        .expect("simulate genuine attention Bundle V8");
    let repeated = fe2o3_gfx950_low_precision::simulator::simulate_bundle_v8(&bundle, &request, 3)
        .expect("repeat genuine attention Bundle V8");
    assert_deterministic_and_close(&actual, &repeated, &expected, 2.0e-3);
}

#[test]
#[ignore = "requires a genuine W6 compiler-produced FP4 GEMM Bundle V8; never use synthetic custody"]
fn fp4_gemm_bundle_v8_is_deterministic_and_matches_the_cpu_oracle() {
    exercise_gemm(
        LowPrecisionFormat::Fp4E2M1,
        "gfx950_fp4_gemm_rust",
        "FE2O3_GFX950_FP4_GEMM_BUNDLE_V8",
    );
}

#[test]
#[ignore = "requires a genuine W6 compiler-produced FP8 GEMM Bundle V8; never use synthetic custody"]
fn fp8_gemm_bundle_v8_is_deterministic_and_matches_the_cpu_oracle() {
    exercise_gemm(
        LowPrecisionFormat::Fp8E4M3,
        "gfx950_fp8_gemm_rust",
        "FE2O3_GFX950_FP8_GEMM_BUNDLE_V8",
    );
}

#[test]
#[ignore = "requires a genuine W6 compiler-produced FP4 attention Bundle V8; never use synthetic custody"]
fn fp4_attention_bundle_v8_is_deterministic_and_matches_the_cpu_oracle() {
    exercise_attention(
        LowPrecisionFormat::Fp4E2M1,
        "gfx950_fp4_attention_rust",
        "FE2O3_GFX950_FP4_ATTENTION_BUNDLE_V8",
    );
}

#[test]
#[ignore = "requires a genuine W6 compiler-produced FP8 attention Bundle V8; never use synthetic custody"]
fn fp8_attention_bundle_v8_is_deterministic_and_matches_the_cpu_oracle() {
    exercise_attention(
        LowPrecisionFormat::Fp8E4M3,
        "gfx950_fp8_attention_rust",
        "FE2O3_GFX950_FP8_ATTENTION_BUNDLE_V8",
    );
}
