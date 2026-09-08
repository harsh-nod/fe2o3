#[test]
fn gfx950_capability_surface_is_type_checked() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass/gfx950_capability_complete.rs");
    tests.pass("tests/ui/pass/gfx950_fp4_mfma_formats.rs");
    tests.pass("tests/ui/pass/kernel_context_tensor_brands.rs");
    tests.pass("tests/ui/pass/kernel_context_gfx950_transpose_scope.rs");
    tests.pass("tests/ui/pass/gfx950_transpose_read_operation_lifetime.rs");
    tests.compile_fail("tests/ui/fail/gfx950_capability_*.rs");
    tests.compile_fail("tests/ui/fail/gfx950_mixed_mfma_rejects_fp8_a_fp4_b.rs");
    tests.compile_fail("tests/ui/fail/kernel_context_tensor_gfx950_cross_kernel.rs");
}
