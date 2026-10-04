#[test]
fn allocation_provenance_cannot_be_forged_by_safe_callers() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/argument_alias/*.rs");
}
