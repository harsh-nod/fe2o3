#[test]
fn protected_row_softmax_authority_is_linear_private_and_exact() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/row_softmax_protected_admission/*.rs");
}
