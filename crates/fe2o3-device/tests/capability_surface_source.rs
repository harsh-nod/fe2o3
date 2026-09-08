const MEMORY_SOURCE: &str = include_str!("../src/capability_memory.rs");
const EXECUTION_SOURCE: &str = include_str!("../src/execution.rs");
const MATH_SOURCE: &str = include_str!("../src/math.rs");
const MATRIX_SOURCE: &str = include_str!("../src/matrix.rs");
const NUMERICAL_SOURCE: &str = include_str!("../src/numerical.rs");
const TENSOR_SOURCE: &str = include_str!("../src/tensor.rs");
const GFX950_SOURCE: &str = include_str!("../src/gfx950.rs");

fn assert_exact_item(source: &str, diagnostic: &str) {
    let attributed = format!("#[rustc_diagnostic_item = \"{diagnostic}\"]");
    assert_eq!(
        source.matches(&attributed).count(),
        1,
        "diagnostic item {diagnostic} must have one exact provider",
    );
}

#[test]
fn exclusive_global_terminals_have_exact_provider_identity() {
    for diagnostic in [
        "fe2o3_device_capability_global_bind_exclusive_read_write_v1",
        "fe2o3_device_capability_global_exclusive_load_v1",
        "fe2o3_device_capability_global_exclusive_store_v1",
    ] {
        assert_exact_item(MEMORY_SOURCE, diagnostic);
    }
    assert!(MEMORY_SOURCE.contains("pub fn __compiler_bind_exclusive_read_write("));
    assert!(MEMORY_SOURCE.contains("physical: &'kernel mut [T]"));
    assert!(MEMORY_SOURCE.contains("GlobalAddressSpace, ExclusiveReadWrite, Brand"));
}

#[test]
fn numerical_policy_has_one_root_issuance_terminal() {
    for diagnostic in [
        "fe2o3_device_strict_ieee_numerical_policy_v1",
        "fe2o3_device_numerical_policy_capability_v1",
        "fe2o3_device_numerical_policy_issue_v1",
    ] {
        assert_exact_item(NUMERICAL_SOURCE, diagnostic);
    }
    assert!(
        NUMERICAL_SOURCE.contains(
            "unreachable!(\"numerical-policy issuance requires authenticated lowering\")"
        )
    );
    assert!(!NUMERICAL_SOURCE.contains("impl Default for NumericalPolicyCapability"));
    assert!(!NUMERICAL_SOURCE.contains("pub fn new("));
}

#[test]
fn global_matrix_view_and_lane_terminals_are_exact() {
    for diagnostic in [
        "fe2o3_device_bf16_mfma_global_matrix_view_v1",
        "fe2o3_device_bf16_mfma_global_matrix_a_load_zero_filled_v1",
        "fe2o3_device_bf16_mfma_global_matrix_b_load_zero_filled_v1",
        "fe2o3_device_f32_accumulator_global_matrix_view_v1",
        "fe2o3_device_f32_accumulator_global_matrix_view_error_v1",
        "fe2o3_device_f32_accumulator_global_matrix_load_lane_v1",
        "fe2o3_device_f32_accumulator_global_matrix_store_lane_v1",
    ] {
        assert_exact_item(TENSOR_SOURCE, diagnostic);
    }
    assert!(TENSOR_SOURCE.contains("bits: &'view Global<'kernel, u16, ReadOnly, GlobalBrand>"));
    assert!(
        TENSOR_SOURCE
            .contains("output: &'view mut Global<'kernel, f32, ExclusiveReadWrite, GlobalBrand>")
    );
    assert!(TENSOR_SOURCE.contains("MatrixBrand: MatrixGlobalAccess<GlobalBrand>"));
}

#[test]
fn policy_bound_and_reusable_capability_types_have_exact_identity() {
    assert_exact_item(MATRIX_SOURCE, "fe2o3_device_policy_matrix_capability_v1");
    assert_exact_item(MATRIX_SOURCE, "fe2o3_device_policy_matrix_bind_v1");
    assert_exact_item(MATH_SOURCE, "fe2o3_device_policy_math_capability_v1");
    assert_exact_item(MATH_SOURCE, "fe2o3_device_policy_math_bind_v1");
    for diagnostic in [
        "fe2o3_device_dynamic_phase_epoch_v1",
        "fe2o3_device_reusable_workgroup_brand_v1",
        "fe2o3_device_reusable_workgroup_v1",
        "fe2o3_device_reusable_workgroup_lds_v1",
        "fe2o3_device_reusable_phase_completion_v1",
    ] {
        assert_exact_item(EXECUTION_SOURCE, diagnostic);
    }
}

#[test]
fn gfx950_capability_terminals_have_exact_provider_identity() {
    for diagnostic in [
        "fe2o3_device_capability_global_store_block_v1",
        "fe2o3_device_policy_gfx950_matrix_capability_v1",
        "fe2o3_device_policy_gfx950_matrix_issue_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_view_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_a_fp4_row_major_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_b_fp4_row_major_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_a_fp8_row_major_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_b_fp8_row_major_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_a_fp4_load_m16k128_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_b_fp4_load_k128n16_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_a_fp8_load_m16k128_v1",
        "fe2o3_device_gfx950_mfma_global_matrix_b_fp8_load_k128n16_v1",
        "fe2o3_device_gfx950_subgroup_wave16_v1",
        "fe2o3_device_gfx950_subgroup_reduce_max_f32_wave16_v1",
        "fe2o3_device_gfx950_subgroup_reduce_sum_f32_wave16_v1",
        "fe2o3_device_gfx950_subgroup_broadcast_f32_wave16_v1",
        "fe2o3_device_gfx950_lds_transpose_tile_issue_v1",
        "fe2o3_device_gfx950_lds_transpose_publish_v1",
        "fe2o3_device_gfx950_mfma_fp4_f32_m16n16k128_v1",
        "fe2o3_device_gfx950_mfma_fp4_fp8_f32_m16n16k128_v1",
        "fe2o3_device_gfx950_mfma_fp8_f32_m16n16k128_v1",
    ] {
        let source = if diagnostic == "fe2o3_device_capability_global_store_block_v1" {
            MEMORY_SOURCE
        } else {
            GFX950_SOURCE
        };
        assert_exact_item(source, diagnostic);
    }
}

#[test]
fn policy_math_owns_every_admitted_math_terminal() {
    for diagnostic in [
        "fe2o3_device_math_sqrt_f32_v1",
        "fe2o3_device_math_fma_f32_v1",
        "fe2o3_device_math_floor_f32_v1",
        "fe2o3_device_math_ceil_f32_v1",
        "fe2o3_device_math_trunc_f32_v1",
        "fe2o3_device_math_roundeven_f32_v1",
        "fe2o3_device_math_sin_f32_v1",
        "fe2o3_device_math_cos_f32_v1",
        "fe2o3_device_math_exp_f32_v1",
        "fe2o3_device_math_exp2_f32_v1",
        "fe2o3_device_math_log_f32_v1",
        "fe2o3_device_math_log2_f32_v1",
        "fe2o3_device_math_log10_f32_v1",
        "fe2o3_device_math_fma_bf16x2_v1",
    ] {
        assert_eq!(
            MATH_SOURCE.matches(diagnostic).count(),
            1,
            "math terminal {diagnostic} must be named by one policy provider",
        );
    }
    assert!(MATH_SOURCE.contains("impl<Brand, Policy: NumericalPolicy> PolicyDeviceMath"));
    assert!(!MATH_SOURCE.contains("#[rustc_diagnostic_item = $diagnostic]\n        pub unsafe fn"));
}
