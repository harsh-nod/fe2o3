use fe2o3_gfx950_low_precision::kernel::{GFX950_BATCHES, GFX950_GRID, GFX950_WORKGROUP};
use std::fs;
use std::path::Path;

#[test]
fn gfx950_kernels_are_safe_attributed_rust_with_typed_operations() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/kernel.rs"))
        .expect("read Rust kernel source");
    assert_eq!(source.matches("#[kernel(").count(), 5);
    assert_eq!(source.matches("typed,").count(), 5);
    assert_eq!(
        source
            .matches("launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])")
            .count(),
        5
    );
    assert_eq!(GFX950_WORKGROUP, [256, 1, 1]);
    assert_eq!(GFX950_GRID, [4, 1, 1]);
    assert_eq!(GFX950_BATCHES, 16);
    assert_eq!(source.matches("let batch = index.get() / 64;").count(), 5);
    assert_eq!(source.matches("Blocked<Index1D, 16, 4>").count(), 5);
    assert_eq!(source.matches("checked_block::<16, 4>").count(), 5);
    assert_eq!(source.matches("get_block_mut").count(), 20);
    assert!(!source.contains("checked_tiled_2d"));
    assert!(!source.contains("get_tiled_2d_mut"));
    // Keep all four original exports and the separately named mixed example.
    for name in [
        "gfx950_fp4_gemm_rust",
        "gfx950_fp8_gemm_rust",
        "gfx950_mixed_fp4_fp8_gemm_rust",
        "gfx950_fp4_attention_rust",
        "gfx950_fp8_attention_rust",
    ] {
        let declaration = format!("pub fn {name}(");
        assert_eq!(source.matches(declaration.as_str()).count(), 1);
    }
    assert!(source.contains("multiply_accumulate_fp4"));
    assert!(source.contains("multiply_accumulate_fp8"));
    assert!(source.contains("Gfx950LdsTransposeTile"));
    assert!(source.contains("stage_k_transposed"));
    assert!(source.contains("read_mfma_fragment"));
    assert!(source.contains("Gfx950Subgroup::current"));
    // Each attention kernel materializes four query rows across all 16 value
    // lanes so the semantic importer sees a fixed, statically ranked graph.
    assert_eq!(source.matches("broadcast_f32::<16>").count(), 2 * 4 * 16);
    assert_eq!(source.matches("value.load_or(").count(), 2 * 16);
    assert!(!source.contains("unsafe"));
    assert!(!source.contains("asm!"));
    assert!(!source.contains("__builtin_amdgcn"));
}

#[test]
fn hip_fixture_is_not_imported_by_the_rust_package() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read package manifest");
    assert!(!manifest.contains("build ="));
    assert!(!manifest.contains("gfx950_low_precision.hip"));
}

#[test]
fn mixed_gemm_keeps_its_own_feature_and_typed_packing_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read package manifest");
    let source = fs::read_to_string(root.join("src/kernel.rs")).expect("read Rust kernel source");
    assert!(manifest.contains("kernel-mixed-fp4-fp8-gemm = []"));
    let normalized: String = source.split_whitespace().collect();
    assert!(normalized.contains(
        "#[cfg(any(not(target_arch=\"amdgpu\"),feature=\"kernel-mixed-fp4-fp8-gemm\"))]"
    ));
    let mixed = source
        .split_once("pub fn gfx950_mixed_fp4_fp8_gemm_rust(")
        .expect("separate mixed export")
        .1
        .split_once("pub fn gfx950_fp4_attention_rust(")
        .expect("following original attention export")
        .0;
    assert!(mixed.contains("lhs: &[u8]"));
    assert!(mixed.contains("rhs: &[u8]"));
    assert!(mixed.contains("DisjointSlice<f32, Blocked<Index1D, 16, 4>>"));
    assert!(mixed.contains("Gfx950Fp4MfmaAMatrix::row_major("));
    assert!(mixed.contains("Gfx950Fp8MfmaBMatrix::row_major("));
    assert!(mixed.contains("Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1>::zero("));
    assert_eq!(mixed.matches(".multiply_accumulate_fp4_fp8(").count(), 1);
    assert_eq!(mixed.matches("get_block_mut").count(), 4);
    // Numerical correctness remains covered by the independent reference tests
    // and the genuine source-to-simulator regression, not this source census.
}
