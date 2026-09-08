use std::fs;
use std::path::Path;

#[test]
fn gfx950_kernels_are_safe_attributed_rust_with_typed_operations() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/kernel.rs"))
        .expect("read Rust kernel source");
    assert_eq!(source.matches("#[kernel(").count(), 4);
    assert_eq!(source.matches("typed,").count(), 4);
    assert_eq!(
        source
            .matches("launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])")
            .count(),
        4
    );
    assert!(source.contains("pub const GFX950_WORKGROUP: [u32; 3] = [256, 1, 1];"));
    assert!(source.contains("pub const GFX950_GRID: [u32; 3] = [4, 1, 1];"));
    assert!(source.contains("pub const GFX950_BATCHES: usize = 16;"));
    assert_eq!(source.matches("mut context: KernelContext<'_>").count(), 4);
    assert_eq!(source.matches("context.invocation().index_1d()").count(), 4);
    assert_eq!(source.matches("context.with_workgroup").count(), 4);
    assert_eq!(
        source
            .matches("workgroup.subgroup::<SubgroupWidth64>()")
            .count(),
        6
    );
    assert_eq!(source.matches("subgroup.with_matrix").count(), 6);
    assert_eq!(source.matches("matrix.with_numerical_policy").count(), 6);
    assert_eq!(source.matches("context.math()").count(), 2);
    assert_eq!(source.matches("let batch = index.get() / 64;").count(), 4);
    assert_eq!(source.matches("Blocked<Index1D, 16, 4>").count(), 4);
    assert_eq!(source.matches("checked_block::<16, 4>").count(), 4);
    assert_eq!(source.matches("output.store_block").count(), 16);
    assert!(!source.contains("checked_tiled_2d"));
    assert!(!source.contains("get_tiled_2d_mut"));
    assert!(source.contains("multiply_accumulate_fp4"));
    assert!(source.contains("multiply_accumulate_fp8"));
    assert_eq!(source.matches(".transpose_tile::<").count(), 2);
    assert_eq!(source.matches("staged.publish(workgroup)").count(), 2);
    assert!(source.contains("stage_k_transposed"));
    assert!(source.contains("read_mfma_fragment"));
    assert_eq!(source.matches(".gfx950_wave16(").count(), 4);
    assert!(!source.contains("thread::index_1d"));
    assert!(!source.contains("WaveLane::<Wave64>::current"));
    assert!(!source.contains("Gfx950Matrix::current"));
    assert!(!source.contains("Math::current"));
    // Each attention kernel materializes four query rows across all 16 value
    // lanes so the semantic importer sees a fixed, statically ranked graph.
    assert_eq!(source.matches("wave16.broadcast_f32").count(), 2 * 4 * 16);
    assert_eq!(source.matches("global_load_2d_u8(").count(), 2 * 16);
    assert!(!source.contains("unsafe"));
    assert!(!source.contains("asm!"));
    assert!(!source.contains("__builtin_amdgcn"));
}

#[test]
fn compiler_roots_have_no_raw_or_compatibility_capability_route() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/kernel.rs"))
        .expect("read Rust kernel source");

    let retained = [
        ": &[u8]",
        "DisjointSlice",
        "Gfx950Subgroup::current",
        ">::current(",
        "StridedReadView2D::from_shared_slice",
        "get_block_mut",
        "let _policy =",
        "let _policy_math =",
        "Gfx950F32AccumulatorFragment",
    ]
    .into_iter()
    .filter(|forbidden| source.contains(forbidden))
    .collect::<Vec<_>>();
    assert!(
        retained.is_empty(),
        "compiler roots retained forbidden source routes: {retained:?}"
    );

    assert_eq!(source.matches("Global<'_, u8, ReadOnly>").count(), 10);
    assert_eq!(
        source
            .matches("Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>")
            .count(),
        4
    );
}

#[test]
fn hip_fixture_is_not_imported_by_the_rust_package() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read package manifest");
    assert!(!manifest.contains("build ="));
    assert!(!manifest.contains("gfx950_low_precision.hip"));
}

#[test]
fn package_uses_the_generic_importer_without_transcript_selectors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = ["Cargo.toml", "src/lib.rs", "src/kernel.rs", "README.md"]
        .into_iter()
        .map(|path| fs::read_to_string(root.join(path)).expect("read package source"))
        .collect::<String>();

    for forbidden in [
        "source_selector",
        "kernel_selector",
        "profile_selector",
        "source-selector",
        "kernel-selector",
        "profile-selector",
        "transcript_selector",
        "transcript-selector",
    ] {
        assert!(
            !source.contains(forbidden),
            "package retained importer selector {forbidden:?}"
        );
    }
}
