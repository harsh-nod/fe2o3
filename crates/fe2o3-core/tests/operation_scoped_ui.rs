#[test]
fn scoped_operation_resources_enforce_typed_borrows() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/operation/fail/*.rs");
    tests.pass("tests/ui/operation/pass/*.rs");
}
