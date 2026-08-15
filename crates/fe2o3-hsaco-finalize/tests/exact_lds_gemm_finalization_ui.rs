#[test]
fn exact_lds_gemm_finalization_authority_is_linear_and_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/exact_lds_gemm_finalization/*.rs");
}
