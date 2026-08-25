#![cfg(feature = "qualification-oracles-test-only")]

#[test]
fn hsa_lifecycle_authority_is_linear_private_and_unsafe_at_review_boundaries() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/hsa_executable_lifecycle/*.rs");
}
