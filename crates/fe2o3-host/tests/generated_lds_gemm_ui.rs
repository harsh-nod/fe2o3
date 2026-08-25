#![cfg(feature = "qualification-oracles-test-only")]

#[test]
fn generated_lds_gemm_capabilities_remain_sealed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/generated_lds_gemm/*.rs");
    cases.pass("tests/ui/generated_lds_gemm/pass/*.rs");
}
