#[test]
fn generated_expectations_and_semantic_witnesses_remain_sealed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/authenticated_artifact/*.rs");
}
