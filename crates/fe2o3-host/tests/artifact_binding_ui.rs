#[test]
fn artifact_validation_cannot_mint_an_arbitrary_marker() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/artifact_binding/*.rs");
}
