#[test]
fn worker_v3_application_handoff_is_opaque_and_distinct_from_worker_v2() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/application_handoff_v3/*.rs");
}
