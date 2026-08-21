#[test]
fn protected_v2_schema_is_opaque_and_distinct_from_v1() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/protected_v2/*.rs");
}
