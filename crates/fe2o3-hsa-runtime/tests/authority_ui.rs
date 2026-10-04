#[test]
fn native_authority_cannot_be_forged() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/authority/*.rs");
}
