#[test]
fn authenticated_generated_artifacts_require_unsafe_issuance_and_remain_sealed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/authenticated_artifact/*.rs");
}
