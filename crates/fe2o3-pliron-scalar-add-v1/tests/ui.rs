//! Compile-fail coverage for the production authority boundary.

#[test]
fn production_authority_is_not_constructible_or_extractable() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
