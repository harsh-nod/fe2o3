use std::fs;
use std::path::Path;

#[test]
fn gfx950_kernels_are_safe_attributed_rust_with_typed_operations() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/kernel.rs"))
        .expect("read Rust kernel source");
    assert_eq!(source.matches("#[kernel(").count(), 4);
    assert_eq!(source.matches("typed,").count(), 4);
    assert!(source.contains("multiply_accumulate_fp4"));
    assert!(source.contains("multiply_accumulate_fp8"));
    assert!(source.contains("Gfx950LdsTransposeTile"));
    assert!(source.contains("stage_k_transposed"));
    assert!(source.contains("read_mfma_fragment"));
    assert!(source.contains("Gfx950Subgroup::current"));
    assert_eq!(source.matches("broadcast_f32::<16>").count(), 8);
    assert_eq!(
        source.matches("value.load_or(token, lane_column").count(),
        2
    );
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
