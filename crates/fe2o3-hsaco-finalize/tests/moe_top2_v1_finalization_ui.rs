#[test]
fn exact_moe_finalization_exposes_no_bytes_or_authority() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/moe_top2_v1_finalization/*.rs");
}
