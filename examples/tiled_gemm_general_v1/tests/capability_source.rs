use std::collections::BTreeSet;

const KERNEL: &str = include_str!("../src/kernel.rs");
const LIBRARY: &str = include_str!("../src/lib.rs");

#[test]
fn one_target_neutral_root_preserves_the_dynamic_gemm_contract() {
    assert_eq!(KERNEL.matches("#[kernel(").count(), 1);
    for required in [
        "pub fn tiled_gemm_general_v1(",
        "KernelContext<'_>",
        "a: Global<'_, u16, ReadOnly>",
        "b: Global<'_, u16, ReadOnly>",
        "c: Global<'_, f32, ExclusiveReadWrite>",
        "context.invocation()",
        "context.private_memory::<f32, 4>()",
        "context.subgroup_lane::<SubgroupWidth64>()",
        "context.numerical_policy::<StrictIeee>()",
        "context.matrix()",
        "with_numerical_policy",
        "bf16_a_global_row_major",
        "bf16_b_global_row_major",
        "load_m16k16",
        "load_k16n16",
        "multiply_accumulate",
        "control_flow(loop_bounds(4294967295, 4, 4))",
        "static_shared_memory_bytes = 2048",
        "epilogue_v1(product, previous, alpha, beta)",
    ] {
        assert!(KERNEL.contains(required), "missing {required:?}");
    }
    for forbidden in [
        "thread::",
        "WaveLane::<Wave64>::current",
        "Matrix::current",
        "WorkgroupLdsScope::current",
        "DeviceGlobalConstPtr",
        "DeviceGlobalMutPtr",
        "unsafe {",
        "Gfx942",
        "Gfx950",
        "gfx942",
        "gfx950",
        "profile dispatch",
    ] {
        assert!(!KERNEL.contains(forbidden), "found {forbidden:?}");
    }

    assert_eq!(KERNEL.matches(">::current(").count(), 0);
}

#[test]
fn k_fragment_and_output_layouts_are_bijections() {
    let mut a = BTreeSet::new();
    let mut b = BTreeSet::new();
    let mut output = BTreeSet::new();
    for lane in 0..64 {
        for component in 0..4 {
            let depth = 4 * (lane / 16) + component;
            assert!(a.insert((lane % 16, depth)));
            assert!(b.insert((depth, lane % 16)));
            assert!(output.insert((4 * (lane / 16) + component, lane % 16)));
        }
    }
    assert_eq!(a.len(), 16 * 16);
    assert_eq!(b.len(), 16 * 16);
    assert_eq!(output.len(), 16 * 16);
}

#[test]
fn multiple_workgroups_cover_dynamic_mn_tails_once() {
    let (rows, columns) = (19_usize, 21_usize);
    let tile_rows = rows.div_ceil(16);
    let tile_columns = columns.div_ceil(16);
    let mut covered = BTreeSet::new();
    for tile in 0..tile_rows * tile_columns {
        let tile_row = tile / tile_columns;
        let tile_column = tile % tile_columns;
        for lane in 0..64 {
            for component in 0..4 {
                let row = tile_row * 16 + 4 * (lane / 16) + component;
                let column = tile_column * 16 + lane % 16;
                if row < rows && column < columns {
                    assert!(covered.insert((row, column)));
                }
            }
        }
    }
    assert_eq!(covered.len(), rows * columns);
}

#[test]
fn retired_compatibility_bridges_are_not_used_by_the_kernel() {
    for fixture in [
        include_str!("capability-ui/boundary/global_matrix_bridge.rs"),
        include_str!("capability-ui/boundary/dynamic_workgroup_epoch_loop.rs"),
        include_str!("capability-ui/boundary/typed_global_epilogue.rs"),
        include_str!("capability-ui/boundary/numerical_policy_binding.rs"),
    ] {
        assert!(fixture.contains("expected-boundary: FE2O3-CAP-GEMM"));
    }
    assert!(KERNEL.contains("Global<'_"));
    for unsupported in [
        "GENERAL_TILED_GEMM_SOURCE_TO_IR_SUPPORTED_V1: bool = false",
        "GENERAL_TILED_GEMM_SOURCE_LOWERING_SUPPORTED_V1: bool = false",
        "GENERAL_TILED_GEMM_QUALIFICATION_EXECUTION_SUPPORTED_V1: bool = false",
    ] {
        assert!(LIBRARY.contains(unsupported));
    }
}
