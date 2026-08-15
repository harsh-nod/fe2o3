#[test]
fn exact_lds_gemm_lifecycle_authority_is_linear_private_and_quiescent() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/generated_lds_gemm_lifecycle/*.rs");
    cases.pass("tests/ui/generated_lds_gemm_lifecycle/pass/*.rs");
}
