//! Compile-fail coverage for linear compiler ownership boundaries.

#[test]
fn linear_compiler_custody_is_not_cloneable_or_publicly_constructible() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
