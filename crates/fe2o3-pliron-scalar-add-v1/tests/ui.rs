//! Compile-fail coverage for the production authority boundary.

#[test]
fn production_authority_is_not_constructible_or_extractable() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/finalize_*.rs");
    tests.compile_fail("tests/ui/policy_*.rs");
    tests.compile_fail("tests/ui/receipt_*.rs");
}

#[cfg(feature = "qualification-oracles-test-only")]
#[test]
fn qualification_runtime_is_one_shot_and_non_extractable() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/runtime_*.rs");
}
